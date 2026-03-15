use std::{
    slice,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, SyncSender},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use crate::error::{AppError, AppResult};

#[allow(dead_code)]
#[derive(Debug)]
pub struct WgcRawCapture {
    pub width: u32,
    pub height: u32,
    pub bgra_top_down: Vec<u8>,
    pub sequence: u64,
    pub captured_at: Instant,
    pub device_create_ms: u128,
    pub item_create_ms: u128,
    pub session_create_ms: u128,
    pub frame_wait_ms: u128,
    pub map_ms: u128,
    pub total_ms: u128,
}

#[allow(dead_code)]
pub struct WgcLiveCaptureStarted {
    pub width: u32,
    pub height: u32,
    pub device_create_ms: u128,
    pub item_create_ms: u128,
    pub session_create_ms: u128,
}

pub struct WgcLiveCaptureHandle {
    stop_tx: SyncSender<()>,
    worker: Option<JoinHandle<()>>,
}

impl WgcLiveCaptureHandle {
    pub fn stop(&mut self) {
        let _ = self.stop_tx.try_send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for WgcLiveCaptureHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::*;
    use windows::{
        core::{factory, IInspectable, Interface},
        Foundation::TypedEventHandler,
        Graphics::{
            Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession},
            DirectX::{Direct3D11::IDirect3DDevice, DirectXPixelFormat},
        },
        Win32::{
            Foundation::{HMODULE, RPC_E_CHANGED_MODE},
            Graphics::{
                Direct3D::{
                    D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_10_0,
                    D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
                },
                Direct3D11::{
                    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
                    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_SDK_VERSION,
                    D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
                },
                Dxgi::{Common::DXGI_SAMPLE_DESC, IDXGIDevice},
                Gdi::HMONITOR,
            },
            System::WinRT::{
                Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess},
                Graphics::Capture::IGraphicsCaptureItemInterop,
                RoInitialize, RO_INIT_MULTITHREADED,
            },
        },
    };

    const WGC_FRAME_TIMEOUT_MS: u64 = 250;

    #[allow(dead_code)]
    struct LiveFrameCallbackState {
        device: ID3D11Device,
        device_context: ID3D11DeviceContext,
        min_frame_interval: Duration,
        last_frame_at: Mutex<Option<Instant>>,
        sequence: AtomicU64,
        staging_texture: Mutex<Option<ID3D11Texture2D>>,
        on_frame: Arc<dyn Fn(WgcRawCapture) + Send + Sync>,
    }

    pub fn capture_monitor_frame(monitor: isize) -> AppResult<WgcRawCapture> {
        let started_at = Instant::now();
        ensure_winrt_initialized()?;

        let device_started_at = Instant::now();
        let (device, device_context) = create_d3d_device()?;
        let winrt_device = create_winrt_device(&device)?;
        let device_create_ms = device_started_at.elapsed().as_millis();

        let item_started_at = Instant::now();
        let capture_item = create_capture_item_for_monitor(monitor)?;
        let item_size = capture_item.Size().map_err(map_windows_error(
            "SCREENSHOT_WGC_ITEM_SIZE_FAILED",
            "读取 WGC 捕获尺寸失败",
        ))?;
        if item_size.Width <= 0 || item_size.Height <= 0 {
            return Err(
                AppError::new("SCREENSHOT_WGC_ITEM_SIZE_INVALID", "WGC 捕获尺寸无效")
                    .with_detail("width", item_size.Width.to_string())
                    .with_detail("height", item_size.Height.to_string()),
            );
        }
        let item_create_ms = item_started_at.elapsed().as_millis();

        let session_started_at = Instant::now();
        let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &winrt_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            1,
            item_size,
        )
        .map_err(map_windows_error(
            "SCREENSHOT_WGC_FRAME_POOL_FAILED",
            "创建 WGC FramePool 失败",
        ))?;
        let session = frame_pool
            .CreateCaptureSession(&capture_item)
            .map_err(map_windows_error(
                "SCREENSHOT_WGC_SESSION_FAILED",
                "创建 WGC Session 失败",
            ))?;
        let _ = session.SetIsCursorCaptureEnabled(false);
        let _ = session.SetIsBorderRequired(false);
        let session_create_ms = session_started_at.elapsed().as_millis();

        let frame_wait_started_at = Instant::now();
        let (frame_tx, frame_rx) = mpsc::sync_channel::<()>(1);
        let handler =
            TypedEventHandler::<Direct3D11CaptureFramePool, IInspectable>::new(move |_, _| {
                let _ = frame_tx.try_send(());
                Ok(())
            });
        let token = frame_pool
            .FrameArrived(&handler)
            .map_err(map_windows_error(
                "SCREENSHOT_WGC_FRAME_EVENT_FAILED",
                "注册 WGC 帧事件失败",
            ))?;

        session.StartCapture().map_err(map_windows_error(
            "SCREENSHOT_WGC_START_FAILED",
            "启动 WGC 捕获失败",
        ))?;

        let wait_result = frame_rx.recv_timeout(Duration::from_millis(WGC_FRAME_TIMEOUT_MS));
        let frame = match frame_pool.TryGetNextFrame() {
            Ok(frame) => frame,
            Err(error) => {
                let _ = frame_pool.RemoveFrameArrived(token);
                let _ = session.Close();
                let _ = frame_pool.Close();
                return match wait_result {
                    Ok(_) => Err(map_windows_error(
                        "SCREENSHOT_WGC_FRAME_FAILED",
                        "读取 WGC 帧失败",
                    )(error)),
                    Err(_) => Err(AppError::new(
                        "SCREENSHOT_WGC_FRAME_TIMEOUT",
                        "等待 WGC 首帧超时",
                    )
                    .with_detail("timeoutMs", WGC_FRAME_TIMEOUT_MS.to_string())),
                };
            }
        };
        let frame_wait_ms = frame_wait_started_at.elapsed().as_millis();

        let map_started_at = Instant::now();
        let surface = frame.Surface().map_err(map_windows_error(
            "SCREENSHOT_WGC_SURFACE_FAILED",
            "读取 WGC 帧表面失败",
        ))?;
        let (width, height, bgra_top_down) =
            map_surface_to_top_down_bgra(&device, &device_context, &surface, None)?;
        let map_ms = map_started_at.elapsed().as_millis();

        let _ = frame.Close();
        let _ = frame_pool.RemoveFrameArrived(token);
        let _ = session.Close();
        let _ = frame_pool.Close();

        Ok(WgcRawCapture {
            width,
            height,
            bgra_top_down,
            sequence: 1,
            captured_at: Instant::now(),
            device_create_ms,
            item_create_ms,
            session_create_ms,
            frame_wait_ms,
            map_ms,
            total_ms: started_at.elapsed().as_millis(),
        })
    }

    #[allow(dead_code)]
    pub fn start_live_capture<F>(
        monitor: isize,
        min_frame_interval: Duration,
        on_frame: F,
    ) -> AppResult<(WgcLiveCaptureHandle, WgcLiveCaptureStarted)>
    where
        F: Fn(WgcRawCapture) + Send + Sync + 'static,
    {
        ensure_winrt_initialized()?;

        let device_started_at = Instant::now();
        let (device, device_context) = create_d3d_device()?;
        let winrt_device = create_winrt_device(&device)?;
        let device_create_ms = device_started_at.elapsed().as_millis();

        let item_started_at = Instant::now();
        let capture_item = create_capture_item_for_monitor(monitor)?;
        let item_size = capture_item.Size().map_err(map_windows_error(
            "SCREENSHOT_WGC_ITEM_SIZE_FAILED",
            "读取 WGC 捕获尺寸失败",
        ))?;
        if item_size.Width <= 0 || item_size.Height <= 0 {
            return Err(
                AppError::new("SCREENSHOT_WGC_ITEM_SIZE_INVALID", "WGC 捕获尺寸无效")
                    .with_detail("width", item_size.Width.to_string())
                    .with_detail("height", item_size.Height.to_string()),
            );
        }
        let item_create_ms = item_started_at.elapsed().as_millis();

        let session_started_at = Instant::now();
        let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &winrt_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            1,
            item_size,
        )
        .map_err(map_windows_error(
            "SCREENSHOT_WGC_FRAME_POOL_FAILED",
            "创建 WGC FramePool 失败",
        ))?;
        let session = frame_pool
            .CreateCaptureSession(&capture_item)
            .map_err(map_windows_error(
                "SCREENSHOT_WGC_SESSION_FAILED",
                "创建 WGC Session 失败",
            ))?;
        let _ = session.SetIsCursorCaptureEnabled(false);
        let _ = session.SetIsBorderRequired(false);
        let session_create_ms = session_started_at.elapsed().as_millis();

        let callback_state = Arc::new(LiveFrameCallbackState {
            device: device.clone(),
            device_context: device_context.clone(),
            min_frame_interval,
            last_frame_at: Mutex::new(None),
            sequence: AtomicU64::new(0),
            staging_texture: Mutex::new(None),
            on_frame: Arc::new(on_frame),
        });
        let handler_state = callback_state.clone();
        let handler =
            TypedEventHandler::<Direct3D11CaptureFramePool, IInspectable>::new(move |sender, _| {
                if let Some(frame_pool) = sender.as_ref() {
                    if let Err(error) = handle_live_frame_arrived(frame_pool, &handler_state) {
                        log::warn!(
                            target: "bexo::service::screenshot",
                            "live_capture_frame_failed reason={}",
                            error
                        );
                    }
                }
                Ok(())
            });
        let token = frame_pool
            .FrameArrived(&handler)
            .map_err(map_windows_error(
                "SCREENSHOT_WGC_FRAME_EVENT_FAILED",
                "注册 WGC 帧事件失败",
            ))?;
        session.StartCapture().map_err(map_windows_error(
            "SCREENSHOT_WGC_START_FAILED",
            "启动 WGC 持续捕获失败",
        ))?;

        let (stop_tx, stop_rx) = mpsc::sync_channel::<()>(1);
        let worker_frame_pool = frame_pool.clone();
        let worker_session = session.clone();
        let worker = std::thread::Builder::new()
            .name("bexo-wgc-live-capture".to_string())
            .spawn(move || {
                let _ = stop_rx.recv();
                let _ = worker_frame_pool.RemoveFrameArrived(token);
                let _ = worker_session.Close();
                let _ = worker_frame_pool.Close();
            })
            .map_err(|error| {
                AppError::new(
                    "SCREENSHOT_WGC_WORKER_START_FAILED",
                    "启动 WGC 持续捕获线程失败",
                )
                .with_detail("reason", error.to_string())
            })?;

        Ok((
            WgcLiveCaptureHandle {
                stop_tx,
                worker: Some(worker),
            },
            WgcLiveCaptureStarted {
                width: item_size.Width as u32,
                height: item_size.Height as u32,
                device_create_ms,
                item_create_ms,
                session_create_ms,
            },
        ))
    }

    #[allow(dead_code)]
    fn handle_live_frame_arrived(
        frame_pool: &Direct3D11CaptureFramePool,
        state: &Arc<LiveFrameCallbackState>,
    ) -> AppResult<()> {
        let frame = match frame_pool.TryGetNextFrame() {
            Ok(frame) => frame,
            Err(_) => return Ok(()),
        };
        let now = Instant::now();
        {
            let last_frame_at = state.last_frame_at.lock().map_err(|_| {
                AppError::new(
                    "SCREENSHOT_WGC_LIVE_LOCK_FAILED",
                    "读取持续捕获节流状态失败",
                )
            })?;
            if let Some(previous) = *last_frame_at {
                if now.duration_since(previous) < state.min_frame_interval {
                    let _ = frame.Close();
                    return Ok(());
                }
            }
        }

        let surface = frame.Surface().map_err(map_windows_error(
            "SCREENSHOT_WGC_SURFACE_FAILED",
            "读取 WGC 持续帧表面失败",
        ))?;
        let (width, height, bgra_top_down) = map_surface_to_top_down_bgra(
            &state.device,
            &state.device_context,
            &surface,
            Some(&state.staging_texture),
        )?;
        let sequence = state.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        {
            let mut last_frame_at = state.last_frame_at.lock().map_err(|_| {
                AppError::new(
                    "SCREENSHOT_WGC_LIVE_LOCK_FAILED",
                    "更新持续捕获节流状态失败",
                )
            })?;
            *last_frame_at = Some(now);
        }
        (state.on_frame)(WgcRawCapture {
            width,
            height,
            bgra_top_down,
            sequence,
            captured_at: now,
            device_create_ms: 0,
            item_create_ms: 0,
            session_create_ms: 0,
            frame_wait_ms: 0,
            map_ms: 0,
            total_ms: 0,
        });
        let _ = frame.Close();
        Ok(())
    }

    fn ensure_winrt_initialized() -> AppResult<()> {
        match unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
            Ok(()) => Ok(()),
            Err(error) if error.code() == RPC_E_CHANGED_MODE => Ok(()),
            Err(error) => Err(map_windows_error(
                "SCREENSHOT_WGC_INIT_FAILED",
                "初始化 WinRT 失败",
            )(error)),
        }
    }

    fn create_d3d_device() -> AppResult<(ID3D11Device, ID3D11DeviceContext)> {
        let mut last_error = None;
        for driver_type in [D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP] {
            let feature_levels = [
                D3D_FEATURE_LEVEL_11_1,
                D3D_FEATURE_LEVEL_11_0,
                D3D_FEATURE_LEVEL_10_1,
                D3D_FEATURE_LEVEL_10_0,
            ];
            let mut device = None;
            let mut device_context = None;
            let result = unsafe {
                D3D11CreateDevice(
                    None,
                    driver_type,
                    HMODULE::default(),
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    Some(&feature_levels),
                    D3D11_SDK_VERSION,
                    Some(&mut device),
                    None,
                    Some(&mut device_context),
                )
            };
            match result {
                Ok(()) => {
                    if let (Some(device), Some(device_context)) = (device, device_context) {
                        return Ok((device, device_context));
                    }
                }
                Err(error) => {
                    last_error = Some(error);
                }
            }
        }

        Err(match last_error {
            Some(error) => map_windows_error(
                "SCREENSHOT_WGC_D3D_DEVICE_FAILED",
                "创建 WGC D3D11 设备失败",
            )(error),
            None => AppError::new(
                "SCREENSHOT_WGC_D3D_DEVICE_FAILED",
                "创建 WGC D3D11 设备失败",
            ),
        })
    }

    fn create_winrt_device(device: &ID3D11Device) -> AppResult<IDirect3DDevice> {
        let dxgi_device: IDXGIDevice = device.cast().map_err(map_windows_error(
            "SCREENSHOT_WGC_DXGI_DEVICE_FAILED",
            "读取 WGC DXGI 设备失败",
        ))?;
        let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device) }.map_err(
            map_windows_error(
                "SCREENSHOT_WGC_WINRT_DEVICE_FAILED",
                "创建 WGC WinRT 设备失败",
            ),
        )?;
        inspectable.cast().map_err(map_windows_error(
            "SCREENSHOT_WGC_WINRT_DEVICE_CAST_FAILED",
            "转换 WGC WinRT 设备失败",
        ))
    }

    fn create_capture_item_for_monitor(monitor: isize) -> AppResult<GraphicsCaptureItem> {
        if !GraphicsCaptureSession::IsSupported().map_err(map_windows_error(
            "SCREENSHOT_WGC_UNSUPPORTED",
            "当前系统不支持 WGC",
        ))? {
            return Err(AppError::new(
                "SCREENSHOT_WGC_UNSUPPORTED",
                "当前系统不支持 WGC",
            ));
        }
        let interop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>().map_err(
            map_windows_error(
                "SCREENSHOT_WGC_INTEROP_FAILED",
                "创建 WGC CaptureItem 工厂失败",
            ),
        )?;
        unsafe { interop.CreateForMonitor::<GraphicsCaptureItem>(HMONITOR(monitor as _)) }.map_err(
            map_windows_error(
                "SCREENSHOT_WGC_CAPTURE_ITEM_FAILED",
                "创建 WGC Monitor CaptureItem 失败",
            ),
        )
    }

    fn map_surface_to_top_down_bgra(
        device: &ID3D11Device,
        device_context: &ID3D11DeviceContext,
        surface: &windows::Graphics::DirectX::Direct3D11::IDirect3DSurface,
        staging_texture_state: Option<&Mutex<Option<ID3D11Texture2D>>>,
    ) -> AppResult<(u32, u32, Vec<u8>)> {
        let access: IDirect3DDxgiInterfaceAccess = surface.cast().map_err(map_windows_error(
            "SCREENSHOT_WGC_SURFACE_CAST_FAILED",
            "转换 WGC DXGI 接口失败",
        ))?;
        let texture: ID3D11Texture2D = unsafe { access.GetInterface() }.map_err(
            map_windows_error("SCREENSHOT_WGC_TEXTURE_FAILED", "读取 WGC 纹理失败"),
        )?;

        let mut texture_desc = D3D11_TEXTURE2D_DESC::default();
        unsafe {
            texture.GetDesc(&mut texture_desc);
        }
        let staging_texture = ensure_staging_texture(device, &texture_desc, staging_texture_state)?;

        unsafe {
            device_context.CopyResource(&staging_texture, &texture);
        }

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            device_context
                .Map(&staging_texture, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .map_err(map_windows_error(
                    "SCREENSHOT_WGC_MAP_FAILED",
                    "映射 WGC 暂存纹理失败",
                ))?;
        }

        let width = texture_desc.Width;
        let height = texture_desc.Height;
        let row_bytes = usize::try_from(width)
            .ok()
            .and_then(|value| value.checked_mul(4))
            .ok_or_else(|| AppError::new("SCREENSHOT_WGC_BUFFER_OVERFLOW", "WGC 行缓冲区溢出"))?;
        let height_usize = usize::try_from(height).map_err(|error| {
            AppError::new("SCREENSHOT_WGC_BUFFER_OVERFLOW", "WGC 高度转换失败")
                .with_detail("reason", error.to_string())
        })?;
        let src_row_pitch = usize::try_from(mapped.RowPitch).map_err(|error| {
            AppError::new("SCREENSHOT_WGC_BUFFER_OVERFLOW", "WGC RowPitch 转换失败")
                .with_detail("reason", error.to_string())
        })?;
        let src_total_bytes = src_row_pitch
            .checked_mul(height_usize)
            .ok_or_else(|| AppError::new("SCREENSHOT_WGC_BUFFER_OVERFLOW", "WGC 源缓冲区溢出"))?;
        let src = unsafe { slice::from_raw_parts(mapped.pData as *const u8, src_total_bytes) };
        let mut bgra_top_down = vec![0u8; row_bytes * height_usize];
        for row in 0..height_usize {
            let src_offset = row * src_row_pitch;
            let dst_offset = row * row_bytes;
            bgra_top_down[dst_offset..dst_offset + row_bytes]
                .copy_from_slice(&src[src_offset..src_offset + row_bytes]);
        }

        unsafe {
            device_context.Unmap(&staging_texture, 0);
        }

        Ok((width, height, bgra_top_down))
    }

    fn ensure_staging_texture(
        device: &ID3D11Device,
        texture_desc: &D3D11_TEXTURE2D_DESC,
        staging_texture_state: Option<&Mutex<Option<ID3D11Texture2D>>>,
    ) -> AppResult<ID3D11Texture2D> {
        if let Some(staging_texture_state) = staging_texture_state {
            let mut guard = staging_texture_state.lock().map_err(|_| {
                AppError::new(
                    "SCREENSHOT_WGC_LIVE_LOCK_FAILED",
                    "读取持续捕获暂存纹理状态失败",
                )
            })?;
            let should_recreate = match guard.as_ref() {
                Some(texture) => {
                    let mut existing_desc = D3D11_TEXTURE2D_DESC::default();
                    unsafe {
                        texture.GetDesc(&mut existing_desc);
                    }
                    existing_desc.Width != texture_desc.Width
                        || existing_desc.Height != texture_desc.Height
                }
                None => true,
            };
            if should_recreate {
                *guard = Some(create_staging_texture(device, texture_desc)?);
            }
            return guard.as_ref().cloned().ok_or_else(|| {
                AppError::new("SCREENSHOT_WGC_STAGING_TEXTURE_FAILED", "WGC 暂存纹理为空")
            });
        }

        create_staging_texture(device, texture_desc)
    }

    fn create_staging_texture(
        device: &ID3D11Device,
        texture_desc: &D3D11_TEXTURE2D_DESC,
    ) -> AppResult<ID3D11Texture2D> {
        let mut staging_desc = *texture_desc;
        staging_desc.Usage = D3D11_USAGE_STAGING;
        staging_desc.BindFlags = 0;
        staging_desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        staging_desc.MiscFlags = 0;
        staging_desc.SampleDesc = DXGI_SAMPLE_DESC {
            Count: texture_desc.SampleDesc.Count.max(1),
            Quality: texture_desc.SampleDesc.Quality,
        };

        let mut staging_texture = None;
        unsafe {
            device
                .CreateTexture2D(&staging_desc, None, Some(&mut staging_texture))
                .map_err(map_windows_error(
                    "SCREENSHOT_WGC_STAGING_TEXTURE_FAILED",
                    "创建 WGC 暂存纹理失败",
                ))?;
        }
        staging_texture.ok_or_else(|| {
            AppError::new("SCREENSHOT_WGC_STAGING_TEXTURE_FAILED", "WGC 暂存纹理为空")
        })
    }

    fn map_windows_error(
        code: &'static str,
        message: &'static str,
    ) -> impl Fn(windows::core::Error) -> AppError {
        move |error| {
            AppError::new(code, message)
                .with_detail("reason", error.to_string())
                .with_detail("hresult", format!("0x{:08X}", error.code().0 as u32))
        }
    }
}

#[cfg(target_os = "windows")]
pub use imp::capture_monitor_frame;

#[cfg(not(target_os = "windows"))]
pub fn capture_monitor_frame(_monitor: isize) -> AppResult<WgcRawCapture> {
    Err(AppError::new(
        "SCREENSHOT_WGC_UNSUPPORTED",
        "当前平台不支持 WGC",
    ))
}

#[cfg(not(target_os = "windows"))]
pub fn start_live_capture<F>(
    _monitor: isize,
    _min_frame_interval: Duration,
    _on_frame: F,
) -> AppResult<(WgcLiveCaptureHandle, WgcLiveCaptureStarted)>
where
    F: Fn(WgcRawCapture) + Send + Sync + 'static,
{
    Err(AppError::new(
        "SCREENSHOT_WGC_UNSUPPORTED",
        "当前平台不支持 WGC 持续捕获",
    ))
}
