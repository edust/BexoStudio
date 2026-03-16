#![cfg(target_os = "windows")]

use std::time::Instant;

use windows::{
    core::{w, Interface},
    Win32::{
        Foundation::{HINSTANCE, HMODULE, HWND, LPARAM, LRESULT, WPARAM},
        Graphics::{
            Direct3D::{
                D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_10_0,
                D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
            },
            Direct3D11::{
                D3D11CreateDevice, ID3D11DepthStencilView, ID3D11Device, ID3D11DeviceContext,
                ID3D11RenderTargetView, ID3D11Texture2D, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                D3D11_SDK_VERSION,
            },
            DirectComposition::{
                DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget,
                IDCompositionVisual,
            },
            Dxgi::{
                Common::{
                    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
                },
                IDXGIAdapter, IDXGIDevice, IDXGIFactory2, IDXGISwapChain1, DXGI_PRESENT,
                DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                DXGI_USAGE_RENDER_TARGET_OUTPUT,
            },
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, LoadCursorW, RegisterClassExW,
            SetWindowPos, ShowWindow, CS_HREDRAW, CS_VREDRAW, HCURSOR, HMENU, HWND_TOPMOST,
            IDC_ARROW, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_HIDE, SW_SHOWNA,
            WNDCLASSEXW, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_NOACTIVATE,
            WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
        },
    },
};

use crate::error::{AppError, AppResult};
use crate::services::native_preview_service::NativePreviewSessionSpec;

const NATIVE_PREVIEW_WINDOW_WIDTH: i32 = 1;
const NATIVE_PREVIEW_WINDOW_HEIGHT: i32 = 1;

#[allow(dead_code)]
#[derive(Debug)]
pub struct NativePreviewWindowsBackend {
    pub device: ID3D11Device,
    pub device_context: ID3D11DeviceContext,
    pub dxgi_device: IDXGIDevice,
    pub dxgi_factory: IDXGIFactory2,
    pub hwnd: HWND,
    pub dcomp_device: IDCompositionDevice,
    pub dcomp_target: IDCompositionTarget,
    pub dcomp_visual: IDCompositionVisual,
    pub swap_chain: IDXGISwapChain1,
    pub current_window_x: i32,
    pub current_window_y: i32,
    pub current_window_width: u32,
    pub current_window_height: u32,
    pub visible: bool,
}

pub struct NativePreviewWindowsBackendStarted {
    pub device_create_ms: u128,
    pub factory_resolve_ms: u128,
    pub window_create_ms: u128,
    pub swap_chain_create_ms: u128,
    pub composition_create_ms: u128,
    pub prime_present_ms: u128,
}

pub fn initialize() -> AppResult<(
    NativePreviewWindowsBackend,
    NativePreviewWindowsBackendStarted,
)> {
    let device_started_at = Instant::now();
    let (device, device_context) = create_d3d_device()?;
    let device_create_ms = device_started_at.elapsed().as_millis();

    let factory_started_at = Instant::now();
    let dxgi_device: IDXGIDevice = device.cast().map_err(map_windows_error(
        "NATIVE_PREVIEW_DXGI_DEVICE_CAST_FAILED",
        "转换 Native Preview DXGI 设备失败",
    ))?;
    let dxgi_factory = resolve_dxgi_factory(&dxgi_device)?;
    let factory_resolve_ms = factory_started_at.elapsed().as_millis();

    let window_started_at = Instant::now();
    let hwnd = create_native_preview_window()?;
    let window_create_ms = window_started_at.elapsed().as_millis();

    let swap_chain_started_at = Instant::now();
    let swap_chain = create_composition_swap_chain(&dxgi_factory, &device)?;
    let swap_chain_create_ms = swap_chain_started_at.elapsed().as_millis();

    let composition_started_at = Instant::now();
    let (dcomp_device, dcomp_target, dcomp_visual) =
        create_direct_composition_tree(&dxgi_device, hwnd, &swap_chain)?;
    let composition_create_ms = composition_started_at.elapsed().as_millis();

    let present_started_at = Instant::now();
    prime_swap_chain(&device, &device_context, &swap_chain)?;
    let prime_present_ms = present_started_at.elapsed().as_millis();

    Ok((
        NativePreviewWindowsBackend {
            device,
            device_context,
            dxgi_device,
            dxgi_factory,
            hwnd,
            dcomp_device,
            dcomp_target,
            dcomp_visual,
            swap_chain,
            current_window_x: 0,
            current_window_y: 0,
            current_window_width: NATIVE_PREVIEW_WINDOW_WIDTH as u32,
            current_window_height: NATIVE_PREVIEW_WINDOW_HEIGHT as u32,
            visible: false,
        },
        NativePreviewWindowsBackendStarted {
            device_create_ms,
            factory_resolve_ms,
            window_create_ms,
            swap_chain_create_ms,
            composition_create_ms,
            prime_present_ms,
        },
    ))
}

impl NativePreviewWindowsBackend {
    pub fn prepare_session(
        &mut self,
        session: &NativePreviewSessionSpec,
        bgra_top_down: &[u8],
    ) -> AppResult<NativePreviewPrepareResult> {
        let started_at = Instant::now();
        let (window_x, window_y, window_width, window_height) = resolve_window_geometry(session)?;
        let resize_started_at = Instant::now();
        self.resize_window_and_swap_chain(window_x, window_y, window_width, window_height)?;
        let resize_ms = resize_started_at.elapsed().as_millis();

        let frame_started_at = Instant::now();
        self.submit_bgra_frame(
            bgra_top_down,
            session.capture_width.max(1),
            session.capture_height.max(1),
        )?;
        let frame_commit_ms = frame_started_at.elapsed().as_millis();

        Ok(NativePreviewPrepareResult {
            resize_ms,
            frame_commit_ms,
            total_ms: started_at.elapsed().as_millis(),
            window_x,
            window_y,
            window_width,
            window_height,
        })
    }

    pub fn show(&mut self) -> AppResult<()> {
        unsafe {
            SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                self.current_window_x,
                self.current_window_y,
                self.current_window_width as i32,
                self.current_window_height as i32,
                SWP_NOACTIVATE,
            )
        }
        .map_err(map_windows_error(
            "NATIVE_PREVIEW_WINDOW_SHOW_POSITION_FAILED",
            "显示 Native Preview 前更新窗口位置失败",
        ))?;
        let _ = unsafe { ShowWindow(self.hwnd, SW_SHOWNA) };
        self.visible = true;
        Ok(())
    }

    pub fn show_below_window(&mut self, anchor_hwnd_raw: isize) -> AppResult<()> {
        let anchor_hwnd = hwnd_from_raw(anchor_hwnd_raw).ok_or_else(|| {
            AppError::new(
                "NATIVE_PREVIEW_ANCHOR_WINDOW_INVALID",
                "Native Preview 锚点窗口句柄无效",
            )
        })?;
        let _ = unsafe { ShowWindow(self.hwnd, SW_SHOWNA) };
        unsafe {
            SetWindowPos(
                self.hwnd,
                Some(anchor_hwnd),
                self.current_window_x,
                self.current_window_y,
                self.current_window_width as i32,
                self.current_window_height as i32,
                SWP_NOACTIVATE,
            )
        }
        .map_err(map_windows_error(
            "NATIVE_PREVIEW_WINDOW_SHOW_ANCHORED_FAILED",
            "显示 Native Preview 并锚定 overlay 层级失败",
        ))?;
        self.visible = true;
        Ok(())
    }

    pub fn hide(&mut self) -> AppResult<()> {
        if !self.visible {
            return Ok(());
        }
        let _ = unsafe { ShowWindow(self.hwnd, SW_HIDE) };
        self.visible = false;
        Ok(())
    }

    pub fn sync_z_order_below_window(&mut self, anchor_hwnd_raw: isize) -> AppResult<()> {
        if !self.visible {
            return Ok(());
        }
        let anchor_hwnd = hwnd_from_raw(anchor_hwnd_raw).ok_or_else(|| {
            AppError::new(
                "NATIVE_PREVIEW_ANCHOR_WINDOW_INVALID",
                "Native Preview 锚点窗口句柄无效",
            )
        })?;
        unsafe {
            SetWindowPos(
                self.hwnd,
                Some(anchor_hwnd),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        }
        .map_err(map_windows_error(
            "NATIVE_PREVIEW_WINDOW_Z_ORDER_SYNC_FAILED",
            "同步 Native Preview 与 overlay 层级失败",
        ))?;
        Ok(())
    }

    fn resize_window_and_swap_chain(
        &mut self,
        window_x: i32,
        window_y: i32,
        window_width: u32,
        window_height: u32,
    ) -> AppResult<()> {
        let size_changed = self.current_window_width != window_width
            || self.current_window_height != window_height;
        let position_changed =
            self.current_window_x != window_x || self.current_window_y != window_y;
        if !size_changed && !position_changed {
            return Ok(());
        }

        let flags = if self.visible {
            SWP_NOACTIVATE
        } else {
            SWP_HIDEWINDOW | SWP_NOACTIVATE
        };
        unsafe {
            SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                window_x,
                window_y,
                window_width as i32,
                window_height as i32,
                flags,
            )
        }
        .map_err(map_windows_error(
            "NATIVE_PREVIEW_WINDOW_RESIZE_FAILED",
            "调整 Native Preview 窗口尺寸失败",
        ))?;

        if size_changed {
            unsafe {
                self.swap_chain.ResizeBuffers(
                    2,
                    window_width,
                    window_height,
                    DXGI_FORMAT_B8G8R8A8_UNORM,
                    windows::Win32::Graphics::Dxgi::DXGI_SWAP_CHAIN_FLAG(0),
                )
            }
            .map_err(map_windows_error(
                "NATIVE_PREVIEW_SWAP_CHAIN_RESIZE_FAILED",
                "调整 Native Preview SwapChain 尺寸失败",
            ))?;
        }

        self.current_window_x = window_x;
        self.current_window_y = window_y;
        self.current_window_width = window_width;
        self.current_window_height = window_height;
        Ok(())
    }

    fn submit_bgra_frame(
        &mut self,
        bgra_top_down: &[u8],
        frame_width: u32,
        frame_height: u32,
    ) -> AppResult<()> {
        let expected_len = usize::try_from(frame_width)
            .ok()
            .and_then(|width| width.checked_mul(usize::try_from(frame_height).ok()?))
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| {
                AppError::new("NATIVE_PREVIEW_FRAME_INVALID", "Native Preview 帧尺寸溢出")
            })?;
        if bgra_top_down.len() != expected_len {
            return Err(AppError::new(
                "NATIVE_PREVIEW_FRAME_INVALID",
                "Native Preview 帧缓冲区尺寸不匹配",
            )
            .with_detail("expected", expected_len.to_string())
            .with_detail("actual", bgra_top_down.len().to_string())
            .with_detail("frameWidth", frame_width.to_string())
            .with_detail("frameHeight", frame_height.to_string()));
        }
        if frame_width != self.current_window_width || frame_height != self.current_window_height {
            return Err(AppError::new(
                "NATIVE_PREVIEW_FRAME_SIZE_MISMATCH",
                "Native Preview 帧尺寸与窗口尺寸不一致",
            )
            .with_detail("frameWidth", frame_width.to_string())
            .with_detail("frameHeight", frame_height.to_string())
            .with_detail("windowWidth", self.current_window_width.to_string())
            .with_detail("windowHeight", self.current_window_height.to_string()));
        }

        let back_buffer: ID3D11Texture2D =
            unsafe { self.swap_chain.GetBuffer(0) }.map_err(map_windows_error(
                "NATIVE_PREVIEW_SWAP_CHAIN_BUFFER_FAILED",
                "读取 Native Preview SwapChain 后备缓冲失败",
            ))?;
        unsafe {
            self.device_context.UpdateSubresource(
                &back_buffer,
                0,
                None,
                bgra_top_down.as_ptr() as *const _,
                frame_width.saturating_mul(4),
                0,
            );
        }
        unsafe { self.swap_chain.Present(0, DXGI_PRESENT(0)) }
            .ok()
            .map_err(map_windows_error(
                "NATIVE_PREVIEW_PRESENT_FAILED",
                "提交 Native Preview 帧失败",
            ))?;
        Ok(())
    }
}

pub struct NativePreviewPrepareResult {
    pub resize_ms: u128,
    pub frame_commit_ms: u128,
    pub total_ms: u128,
    pub window_x: i32,
    pub window_y: i32,
    pub window_width: u32,
    pub window_height: u32,
}

fn resolve_window_geometry(session: &NativePreviewSessionSpec) -> AppResult<(i32, i32, u32, u32)> {
    if !(session.scale_factor.is_finite() && session.scale_factor > 0.0) {
        return Err(AppError::validation("Native Preview 缩放因子无效"));
    }

    let window_width = session.capture_width.max(1);
    let window_height = session.capture_height.max(1);
    let scale_factor = f64::from(session.scale_factor);
    let window_x = (f64::from(session.display_x) * scale_factor).round();
    let window_y = (f64::from(session.display_y) * scale_factor).round();

    if !window_x.is_finite() || !window_y.is_finite() {
        return Err(AppError::new(
            "NATIVE_PREVIEW_WINDOW_GEOMETRY_INVALID",
            "Native Preview 窗口坐标无效",
        ));
    }

    let window_x = window_x.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32;
    let window_y = window_y.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32;

    Ok((window_x, window_y, window_width, window_height))
}

fn hwnd_from_raw(raw: isize) -> Option<HWND> {
    if raw == 0 {
        return None;
    }
    Some(HWND(raw as *mut _))
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
            "NATIVE_PREVIEW_D3D_DEVICE_FAILED",
            "创建 Native Preview D3D11 设备失败",
        )(error),
        None => AppError::new(
            "NATIVE_PREVIEW_D3D_DEVICE_FAILED",
            "创建 Native Preview D3D11 设备失败",
        ),
    })
}

fn resolve_dxgi_factory(dxgi_device: &IDXGIDevice) -> AppResult<IDXGIFactory2> {
    let adapter: IDXGIAdapter = unsafe { dxgi_device.GetAdapter() }.map_err(map_windows_error(
        "NATIVE_PREVIEW_DXGI_ADAPTER_FAILED",
        "读取 Native Preview DXGI 适配器失败",
    ))?;
    unsafe { adapter.GetParent() }.map_err(map_windows_error(
        "NATIVE_PREVIEW_DXGI_FACTORY_FAILED",
        "读取 Native Preview DXGI Factory 失败",
    ))
}

fn create_native_preview_window() -> AppResult<HWND> {
    let module = unsafe { GetModuleHandleW(None) }.map_err(map_windows_error(
        "NATIVE_PREVIEW_MODULE_HANDLE_FAILED",
        "读取 Native Preview 模块句柄失败",
    ))?;
    let instance = HINSTANCE(module.0);
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW) }.map_err(map_windows_error(
        "NATIVE_PREVIEW_CURSOR_LOAD_FAILED",
        "加载 Native Preview 光标失败",
    ))?;
    register_native_preview_window_class(instance, cursor)?;

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP,
            w!("BexoStudioNativePreviewWindow"),
            w!("Bexo Studio Native Preview"),
            WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
            0,
            0,
            NATIVE_PREVIEW_WINDOW_WIDTH,
            NATIVE_PREVIEW_WINDOW_HEIGHT,
            None,
            None::<HMENU>,
            Some(instance),
            None,
        )
    }
    .map_err(map_windows_error(
        "NATIVE_PREVIEW_WINDOW_CREATE_FAILED",
        "创建 Native Preview 窗口失败",
    ))?;

    unsafe {
        SetWindowPos(
            hwnd,
            None,
            0,
            0,
            NATIVE_PREVIEW_WINDOW_WIDTH,
            NATIVE_PREVIEW_WINDOW_HEIGHT,
            SWP_HIDEWINDOW | SWP_NOACTIVATE,
        )
    }
    .map_err(map_windows_error(
        "NATIVE_PREVIEW_WINDOW_POSITION_FAILED",
        "初始化 Native Preview 窗口位置失败",
    ))?;

    Ok(hwnd)
}

fn register_native_preview_window_class(instance: HINSTANCE, cursor: HCURSOR) -> AppResult<()> {
    let class = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(native_preview_window_proc),
        hInstance: instance,
        hCursor: cursor,
        lpszClassName: w!("BexoStudioNativePreviewWindow"),
        ..Default::default()
    };

    let class_atom = unsafe { RegisterClassExW(&class) };
    if class_atom == 0 {
        return Err(AppError::new(
            "NATIVE_PREVIEW_WINDOW_CLASS_FAILED",
            "注册 Native Preview 窗口类失败",
        ));
    }

    Ok(())
}

fn create_composition_swap_chain(
    factory: &IDXGIFactory2,
    device: &ID3D11Device,
) -> AppResult<IDXGISwapChain1> {
    let desc = DXGI_SWAP_CHAIN_DESC1 {
        Width: NATIVE_PREVIEW_WINDOW_WIDTH as u32,
        Height: NATIVE_PREVIEW_WINDOW_HEIGHT as u32,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        Stereo: false.into(),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        Scaling: DXGI_SCALING_STRETCH,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
        Flags: 0,
    };

    unsafe {
        factory.CreateSwapChainForComposition(
            device,
            &desc,
            Option::<&windows::Win32::Graphics::Dxgi::IDXGIOutput>::None,
        )
    }
    .map_err(map_windows_error(
        "NATIVE_PREVIEW_SWAP_CHAIN_FAILED",
        "创建 Native Preview SwapChain 失败",
    ))
}

fn create_direct_composition_tree(
    dxgi_device: &IDXGIDevice,
    hwnd: HWND,
    swap_chain: &IDXGISwapChain1,
) -> AppResult<(
    IDCompositionDevice,
    IDCompositionTarget,
    IDCompositionVisual,
)> {
    let dcomp_device = unsafe { DCompositionCreateDevice::<_, IDCompositionDevice>(dxgi_device) }
        .map_err(map_windows_error(
        "NATIVE_PREVIEW_DCOMP_DEVICE_FAILED",
        "创建 Native Preview DirectComposition 设备失败",
    ))?;
    let dcomp_target =
        unsafe { dcomp_device.CreateTargetForHwnd(hwnd, true) }.map_err(map_windows_error(
            "NATIVE_PREVIEW_DCOMP_TARGET_FAILED",
            "创建 Native Preview DirectComposition Target 失败",
        ))?;
    let dcomp_visual = unsafe { dcomp_device.CreateVisual() }.map_err(map_windows_error(
        "NATIVE_PREVIEW_DCOMP_VISUAL_FAILED",
        "创建 Native Preview DirectComposition Visual 失败",
    ))?;

    unsafe { dcomp_visual.SetContent(swap_chain) }.map_err(map_windows_error(
        "NATIVE_PREVIEW_DCOMP_SET_CONTENT_FAILED",
        "绑定 Native Preview SwapChain 到 Visual 失败",
    ))?;
    unsafe { dcomp_target.SetRoot(&dcomp_visual) }.map_err(map_windows_error(
        "NATIVE_PREVIEW_DCOMP_SET_ROOT_FAILED",
        "绑定 Native Preview Visual 到 Target 失败",
    ))?;
    unsafe { dcomp_device.Commit() }.map_err(map_windows_error(
        "NATIVE_PREVIEW_DCOMP_COMMIT_FAILED",
        "提交 Native Preview DirectComposition 树失败",
    ))?;

    Ok((dcomp_device, dcomp_target, dcomp_visual))
}

fn prime_swap_chain(
    device: &ID3D11Device,
    device_context: &ID3D11DeviceContext,
    swap_chain: &IDXGISwapChain1,
) -> AppResult<()> {
    let back_buffer: ID3D11Texture2D =
        unsafe { swap_chain.GetBuffer(0) }.map_err(map_windows_error(
            "NATIVE_PREVIEW_SWAP_CHAIN_BUFFER_FAILED",
            "读取 Native Preview SwapChain 后备缓冲失败",
        ))?;
    let mut render_target_view: Option<ID3D11RenderTargetView> = None;
    unsafe { device.CreateRenderTargetView(&back_buffer, None, Some(&mut render_target_view)) }
        .map_err(map_windows_error(
            "NATIVE_PREVIEW_RTV_CREATE_FAILED",
            "创建 Native Preview RenderTargetView 失败",
        ))?;
    let render_target_view = render_target_view.ok_or_else(|| {
        AppError::new(
            "NATIVE_PREVIEW_RTV_CREATE_FAILED",
            "创建 Native Preview RenderTargetView 失败",
        )
    })?;

    let render_targets = [Some(render_target_view.clone())];
    unsafe {
        device_context.OMSetRenderTargets(Some(&render_targets), None::<&ID3D11DepthStencilView>);
        device_context.ClearRenderTargetView(&render_target_view, &[0.0, 0.0, 0.0, 0.0]);
    }
    unsafe { swap_chain.Present(0, DXGI_PRESENT(0)) }
        .ok()
        .map_err(map_windows_error(
            "NATIVE_PREVIEW_PRESENT_FAILED",
            "提交 Native Preview 首帧失败",
        ))?;

    Ok(())
}

fn map_windows_error(
    code: &'static str,
    message: &'static str,
) -> impl FnOnce(windows::core::Error) -> AppError {
    move |error| {
        AppError::new(code, message)
            .with_detail("hresult", format!("0x{:08x}", error.code().0 as u32))
            .with_detail("reason", error.message().to_string())
    }
}

unsafe extern "system" fn native_preview_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

impl Drop for NativePreviewWindowsBackend {
    fn drop(&mut self) {
        if self.hwnd.0.is_null() {
            return;
        }

        let _ = unsafe { ShowWindow(self.hwnd, windows::Win32::UI::WindowsAndMessaging::SW_HIDE) };
        let _ = unsafe { DestroyWindow(self.hwnd) };
        self.hwnd = HWND::default();
    }
}
