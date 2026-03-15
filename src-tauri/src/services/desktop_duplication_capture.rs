use std::{
    slice,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use crate::error::{AppError, AppResult};

#[derive(Debug)]
pub struct DesktopDuplicationRawCapture {
    pub width: u32,
    pub height: u32,
    pub bgra_top_down: Vec<u8>,
    pub sequence: u64,
    pub captured_at: Instant,
    pub frame_wait_ms: u128,
    pub map_ms: u128,
    pub total_ms: u128,
}

pub struct DesktopDuplicationLiveCaptureStarted {
    pub width: u32,
    pub height: u32,
    pub factory_create_ms: u128,
    pub adapter_match_ms: u128,
    pub device_create_ms: u128,
    pub duplication_create_ms: u128,
}

pub struct DesktopDuplicationLiveCaptureHandle {
    stop_tx: SyncSender<()>,
    worker: Option<JoinHandle<()>>,
}

impl DesktopDuplicationLiveCaptureHandle {
    pub fn stop(&mut self) {
        let _ = self.stop_tx.try_send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for DesktopDuplicationLiveCaptureHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::*;
    use windows::{
        core::Interface,
        Win32::{
            Foundation::HMODULE,
            Graphics::{
                Direct3D::{
                    D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_10_0, D3D_FEATURE_LEVEL_10_1,
                    D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
                },
                Direct3D11::{
                    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
                    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    D3D11_CREATE_DEVICE_SINGLETHREADED, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ,
                    D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
                },
                Dxgi::{
                    Common::DXGI_SAMPLE_DESC, CreateDXGIFactory1, IDXGIAdapter, IDXGIAdapter1,
                    IDXGIFactory1, IDXGIOutput, IDXGIOutput1, IDXGIOutputDuplication,
                    IDXGIResource, DXGI_OUTDUPL_DESC, DXGI_OUTDUPL_FRAME_INFO, DXGI_OUTPUT_DESC,
                },
                Gdi::HMONITOR,
            },
        },
    };

    const DD_FRAME_TIMEOUT_MS: u32 = 33;
    const DD_STARTUP_TIMEOUT_MS: u64 = 3_000;
    const DXGI_ERROR_WAIT_TIMEOUT: u32 = 0x887A0027;
    const DXGI_ERROR_ACCESS_LOST: u32 = 0x887A0026;
    const DXGI_ERROR_NOT_FOUND: u32 = 0x887A0002;

    struct DesktopDuplicationContext {
        duplication: IDXGIOutputDuplication,
        device: ID3D11Device,
        device_context: ID3D11DeviceContext,
        staging_texture: Option<ID3D11Texture2D>,
    }

    struct LiveFrameCallbackState {
        min_frame_interval: Duration,
        last_frame_at: Mutex<Option<Instant>>,
        sequence: AtomicU64,
        on_frame: Arc<dyn Fn(DesktopDuplicationRawCapture) + Send + Sync>,
    }

    pub fn start_live_capture<F>(
        monitor: isize,
        min_frame_interval: Duration,
        on_frame: F,
    ) -> AppResult<(
        DesktopDuplicationLiveCaptureHandle,
        DesktopDuplicationLiveCaptureStarted,
    )>
    where
        F: Fn(DesktopDuplicationRawCapture) + Send + Sync + 'static,
    {
        let (stop_tx, stop_rx) = mpsc::sync_channel::<()>(1);
        let (startup_tx, startup_rx) =
            mpsc::sync_channel::<AppResult<DesktopDuplicationLiveCaptureStarted>>(1);
        let callback_state = Arc::new(LiveFrameCallbackState {
            min_frame_interval,
            last_frame_at: Mutex::new(None),
            sequence: AtomicU64::new(0),
            on_frame: Arc::new(on_frame),
        });
        let worker_state = callback_state.clone();

        let worker = std::thread::Builder::new()
            .name("bexo-desktop-duplication-live-capture".to_string())
            .spawn(move || {
                run_live_capture_worker(monitor, stop_rx, startup_tx, worker_state);
            })
            .map_err(|error| {
                AppError::new(
                    "SCREENSHOT_DESKTOP_DUPLICATION_WORKER_START_FAILED",
                    "启动 Desktop Duplication 持续捕获线程失败",
                )
                .with_detail("reason", error.to_string())
            })?;

        let started = startup_rx
            .recv_timeout(Duration::from_millis(DD_STARTUP_TIMEOUT_MS))
            .map_err(|error| {
                AppError::new(
                    "SCREENSHOT_DESKTOP_DUPLICATION_START_TIMEOUT",
                    "等待 Desktop Duplication 持续捕获启动超时",
                )
                .with_detail("timeoutMs", DD_STARTUP_TIMEOUT_MS.to_string())
                .with_detail("reason", error.to_string())
            })??;

        Ok((
            DesktopDuplicationLiveCaptureHandle {
                stop_tx,
                worker: Some(worker),
            },
            started,
        ))
    }

    fn run_live_capture_worker(
        monitor: isize,
        stop_rx: Receiver<()>,
        startup_tx: SyncSender<AppResult<DesktopDuplicationLiveCaptureStarted>>,
        state: Arc<LiveFrameCallbackState>,
    ) {
        let mut context = match initialize_duplication_context(monitor) {
            Ok((context, started)) => {
                let _ = startup_tx.send(Ok(started));
                context
            }
            Err(error) => {
                let _ = startup_tx.send(Err(error));
                return;
            }
        };

        loop {
            match stop_rx.try_recv() {
                Ok(_) | Err(TryRecvError::Disconnected) => break,
                Err(TryRecvError::Empty) => {}
            }

            match capture_next_frame(&mut context, &state) {
                Ok(CaptureLoopOutcome::Delivered) | Ok(CaptureLoopOutcome::Skipped) => {}
                Ok(CaptureLoopOutcome::Timeout) => {}
                Err(error) => {
                    log::warn!(
                        target: "bexo::service::screenshot",
                        "desktop_duplication_live_capture_stopped reason={}",
                        error
                    );
                    break;
                }
            }
        }
    }

    enum CaptureLoopOutcome {
        Delivered,
        Skipped,
        Timeout,
    }

    fn initialize_duplication_context(
        monitor: isize,
    ) -> AppResult<(
        DesktopDuplicationContext,
        DesktopDuplicationLiveCaptureStarted,
    )> {
        let factory_started_at = Instant::now();
        let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.map_err(map_windows_error(
            "SCREENSHOT_DESKTOP_DUPLICATION_FACTORY_FAILED",
            "创建 DXGI Factory 失败",
        ))?;
        let factory_create_ms = factory_started_at.elapsed().as_millis();

        let adapter_match_started_at = Instant::now();
        let (adapter, output) = find_output_for_monitor(&factory, monitor)?;
        let adapter_match_ms = adapter_match_started_at.elapsed().as_millis();

        let device_started_at = Instant::now();
        let (device, device_context) = create_d3d_device_for_adapter(&adapter)?;
        let device_create_ms = device_started_at.elapsed().as_millis();

        let duplication_started_at = Instant::now();
        let output1: IDXGIOutput1 = output.cast().map_err(map_windows_error(
            "SCREENSHOT_DESKTOP_DUPLICATION_OUTPUT_CAST_FAILED",
            "转换 Desktop Duplication 输出失败",
        ))?;
        let duplication =
            unsafe { output1.DuplicateOutput(&device) }.map_err(map_windows_error(
                "SCREENSHOT_DESKTOP_DUPLICATION_CREATE_FAILED",
                "创建 Desktop Duplication 会话失败",
            ))?;
        let duplication_create_ms = duplication_started_at.elapsed().as_millis();

        let duplication_desc: DXGI_OUTDUPL_DESC = unsafe { duplication.GetDesc() };

        let width = duplication_desc.ModeDesc.Width.max(1);
        let height = duplication_desc.ModeDesc.Height.max(1);

        Ok((
            DesktopDuplicationContext {
                duplication,
                device,
                device_context,
                staging_texture: None,
            },
            DesktopDuplicationLiveCaptureStarted {
                width,
                height,
                factory_create_ms,
                adapter_match_ms,
                device_create_ms,
                duplication_create_ms,
            },
        ))
    }

    fn find_output_for_monitor(
        factory: &IDXGIFactory1,
        monitor: isize,
    ) -> AppResult<(IDXGIAdapter1, IDXGIOutput)> {
        let target_monitor = HMONITOR(monitor as _);
        let mut adapter_index = 0;
        loop {
            let adapter = match unsafe { factory.EnumAdapters1(adapter_index) } {
                Ok(adapter) => adapter,
                Err(error) => {
                    if error.code().0 as u32 == DXGI_ERROR_NOT_FOUND {
                        break;
                    }
                    return Err(map_windows_error(
                        "SCREENSHOT_DESKTOP_DUPLICATION_ENUM_ADAPTER_FAILED",
                        "枚举 DXGI 适配器失败",
                    )(error));
                }
            };

            let mut output_index = 0;
            loop {
                let output = match unsafe { adapter.EnumOutputs(output_index) } {
                    Ok(output) => output,
                    Err(error) => {
                        if error.code().0 as u32 == DXGI_ERROR_NOT_FOUND {
                            break;
                        }
                        return Err(map_windows_error(
                            "SCREENSHOT_DESKTOP_DUPLICATION_ENUM_OUTPUT_FAILED",
                            "枚举 DXGI 输出失败",
                        )(error));
                    }
                };

                let output_desc: DXGI_OUTPUT_DESC = unsafe {
                    output.GetDesc().map_err(map_windows_error(
                        "SCREENSHOT_DESKTOP_DUPLICATION_OUTPUT_DESC_FAILED",
                        "读取 DXGI 输出描述失败",
                    ))?
                };

                if output_desc.Monitor == target_monitor {
                    return Ok((adapter, output));
                }

                output_index += 1;
            }

            adapter_index += 1;
        }

        Err(AppError::new(
            "SCREENSHOT_DESKTOP_DUPLICATION_OUTPUT_NOT_FOUND",
            "未找到匹配目标显示器的 DXGI 输出",
        )
        .with_detail("monitor", format!("{monitor}")))
    }

    fn create_d3d_device_for_adapter(
        adapter: &IDXGIAdapter1,
    ) -> AppResult<(ID3D11Device, ID3D11DeviceContext)> {
        let adapter: IDXGIAdapter = adapter.cast().map_err(map_windows_error(
            "SCREENSHOT_DESKTOP_DUPLICATION_ADAPTER_CAST_FAILED",
            "转换 DXGI 适配器失败",
        ))?;
        let feature_levels = [
            D3D_FEATURE_LEVEL_11_1,
            D3D_FEATURE_LEVEL_11_0,
            D3D_FEATURE_LEVEL_10_1,
            D3D_FEATURE_LEVEL_10_0,
        ];
        let mut device = None;
        let mut device_context = None;
        unsafe {
            D3D11CreateDevice(
                Some(&adapter),
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_SINGLETHREADED,
                Some(&feature_levels),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut device_context),
            )
            .map_err(map_windows_error(
                "SCREENSHOT_DESKTOP_DUPLICATION_D3D_DEVICE_FAILED",
                "创建 Desktop Duplication D3D11 设备失败",
            ))?;
        }

        match (device, device_context) {
            (Some(device), Some(device_context)) => Ok((device, device_context)),
            _ => Err(AppError::new(
                "SCREENSHOT_DESKTOP_DUPLICATION_D3D_DEVICE_FAILED",
                "Desktop Duplication D3D11 设备为空",
            )),
        }
    }

    fn capture_next_frame(
        context: &mut DesktopDuplicationContext,
        state: &Arc<LiveFrameCallbackState>,
    ) -> AppResult<CaptureLoopOutcome> {
        let started_at = Instant::now();
        let frame_wait_started_at = Instant::now();
        let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut frame_resource: Option<IDXGIResource> = None;
        match unsafe {
            context.duplication.AcquireNextFrame(
                DD_FRAME_TIMEOUT_MS,
                &mut frame_info,
                &mut frame_resource,
            )
        } {
            Ok(()) => {}
            Err(error) if error.code().0 as u32 == DXGI_ERROR_WAIT_TIMEOUT => {
                return Ok(CaptureLoopOutcome::Timeout);
            }
            Err(error) if error.code().0 as u32 == DXGI_ERROR_ACCESS_LOST => {
                return Err(map_windows_error(
                    "SCREENSHOT_DESKTOP_DUPLICATION_ACCESS_LOST",
                    "Desktop Duplication 捕获会话已失效",
                )(error));
            }
            Err(error) => {
                return Err(map_windows_error(
                    "SCREENSHOT_DESKTOP_DUPLICATION_FRAME_FAILED",
                    "Desktop Duplication 获取帧失败",
                )(error));
            }
        }
        let frame_wait_ms = frame_wait_started_at.elapsed().as_millis();

        let now = Instant::now();
        {
            let last_frame_at = state.last_frame_at.lock().map_err(|_| {
                AppError::new(
                    "SCREENSHOT_DESKTOP_DUPLICATION_LOCK_FAILED",
                    "读取 Desktop Duplication 节流状态失败",
                )
            })?;
            if let Some(previous) = *last_frame_at {
                if now.duration_since(previous) < state.min_frame_interval {
                    unsafe {
                        let _ = context.duplication.ReleaseFrame();
                    }
                    return Ok(CaptureLoopOutcome::Skipped);
                }
            }
        }

        let frame_resource = frame_resource.ok_or_else(|| {
            AppError::new(
                "SCREENSHOT_DESKTOP_DUPLICATION_FRAME_EMPTY",
                "Desktop Duplication 帧资源为空",
            )
        })?;
        let texture: ID3D11Texture2D = frame_resource.cast().map_err(map_windows_error(
            "SCREENSHOT_DESKTOP_DUPLICATION_TEXTURE_CAST_FAILED",
            "转换 Desktop Duplication 帧纹理失败",
        ))?;
        let map_started_at = Instant::now();
        let (width, height, bgra_top_down) = map_texture_to_top_down_bgra(
            &context.device,
            &context.device_context,
            &texture,
            &mut context.staging_texture,
        )?;
        let map_ms = map_started_at.elapsed().as_millis();

        unsafe {
            context
                .duplication
                .ReleaseFrame()
                .map_err(map_windows_error(
                    "SCREENSHOT_DESKTOP_DUPLICATION_RELEASE_FAILED",
                    "释放 Desktop Duplication 帧失败",
                ))?;
        }

        {
            let mut last_frame_at = state.last_frame_at.lock().map_err(|_| {
                AppError::new(
                    "SCREENSHOT_DESKTOP_DUPLICATION_LOCK_FAILED",
                    "更新 Desktop Duplication 节流状态失败",
                )
            })?;
            *last_frame_at = Some(now);
        }

        let sequence = state.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        (state.on_frame)(DesktopDuplicationRawCapture {
            width,
            height,
            bgra_top_down,
            sequence,
            captured_at: now,
            frame_wait_ms,
            map_ms,
            total_ms: started_at.elapsed().as_millis(),
        });

        Ok(CaptureLoopOutcome::Delivered)
    }

    fn map_texture_to_top_down_bgra(
        device: &ID3D11Device,
        device_context: &ID3D11DeviceContext,
        texture: &ID3D11Texture2D,
        staging_texture: &mut Option<ID3D11Texture2D>,
    ) -> AppResult<(u32, u32, Vec<u8>)> {
        let mut texture_desc = D3D11_TEXTURE2D_DESC::default();
        unsafe {
            texture.GetDesc(&mut texture_desc);
        }

        if needs_new_staging_texture(staging_texture.as_ref(), &texture_desc) {
            *staging_texture = Some(create_staging_texture(device, &texture_desc)?);
        }
        let staging_texture = staging_texture.as_ref().ok_or_else(|| {
            AppError::new(
                "SCREENSHOT_DESKTOP_DUPLICATION_STAGING_FAILED",
                "Desktop Duplication 暂存纹理为空",
            )
        })?;

        unsafe {
            device_context.CopyResource(staging_texture, texture);
        }

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            device_context
                .Map(staging_texture, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .map_err(map_windows_error(
                    "SCREENSHOT_DESKTOP_DUPLICATION_MAP_FAILED",
                    "映射 Desktop Duplication 暂存纹理失败",
                ))?;
        }

        let width = texture_desc.Width.max(1);
        let height = texture_desc.Height.max(1);
        let width_usize = usize::try_from(width).map_err(|error| {
            AppError::new(
                "SCREENSHOT_DESKTOP_DUPLICATION_BUFFER_FAILED",
                "Desktop Duplication 宽度转换失败",
            )
            .with_detail("reason", error.to_string())
        })?;
        let height_usize = usize::try_from(height).map_err(|error| {
            AppError::new(
                "SCREENSHOT_DESKTOP_DUPLICATION_BUFFER_FAILED",
                "Desktop Duplication 高度转换失败",
            )
            .with_detail("reason", error.to_string())
        })?;
        let row_bytes = width_usize.checked_mul(4).ok_or_else(|| {
            AppError::new(
                "SCREENSHOT_DESKTOP_DUPLICATION_BUFFER_FAILED",
                "Desktop Duplication 行缓冲区溢出",
            )
        })?;
        let src_row_pitch = usize::try_from(mapped.RowPitch).map_err(|error| {
            AppError::new(
                "SCREENSHOT_DESKTOP_DUPLICATION_BUFFER_FAILED",
                "Desktop Duplication RowPitch 转换失败",
            )
            .with_detail("reason", error.to_string())
        })?;
        let src_total_bytes = src_row_pitch.checked_mul(height_usize).ok_or_else(|| {
            AppError::new(
                "SCREENSHOT_DESKTOP_DUPLICATION_BUFFER_FAILED",
                "Desktop Duplication 源缓冲区溢出",
            )
        })?;
        let source = unsafe { slice::from_raw_parts(mapped.pData as *const u8, src_total_bytes) };
        let mut bgra_top_down = vec![0u8; row_bytes * height_usize];
        for row in 0..height_usize {
            let src_offset = row * src_row_pitch;
            let dst_offset = row * row_bytes;
            bgra_top_down[dst_offset..dst_offset + row_bytes]
                .copy_from_slice(&source[src_offset..src_offset + row_bytes]);
        }

        unsafe {
            device_context.Unmap(staging_texture, 0);
        }

        Ok((width, height, bgra_top_down))
    }

    fn needs_new_staging_texture(
        existing: Option<&ID3D11Texture2D>,
        source_desc: &D3D11_TEXTURE2D_DESC,
    ) -> bool {
        let Some(existing) = existing else {
            return true;
        };

        let mut existing_desc = D3D11_TEXTURE2D_DESC::default();
        unsafe {
            existing.GetDesc(&mut existing_desc);
        }
        existing_desc.Width != source_desc.Width || existing_desc.Height != source_desc.Height
    }

    fn create_staging_texture(
        device: &ID3D11Device,
        source_desc: &D3D11_TEXTURE2D_DESC,
    ) -> AppResult<ID3D11Texture2D> {
        let mut staging_desc = *source_desc;
        staging_desc.Usage = D3D11_USAGE_STAGING;
        staging_desc.BindFlags = 0;
        staging_desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        staging_desc.MiscFlags = 0;
        staging_desc.SampleDesc = DXGI_SAMPLE_DESC {
            Count: source_desc.SampleDesc.Count.max(1),
            Quality: source_desc.SampleDesc.Quality,
        };

        let mut staging_texture = None;
        unsafe {
            device
                .CreateTexture2D(&staging_desc, None, Some(&mut staging_texture))
                .map_err(map_windows_error(
                    "SCREENSHOT_DESKTOP_DUPLICATION_STAGING_FAILED",
                    "创建 Desktop Duplication 暂存纹理失败",
                ))?;
        }

        staging_texture.ok_or_else(|| {
            AppError::new(
                "SCREENSHOT_DESKTOP_DUPLICATION_STAGING_FAILED",
                "Desktop Duplication 暂存纹理为空",
            )
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
pub use imp::start_live_capture;
