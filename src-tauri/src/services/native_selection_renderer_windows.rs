// Owner-thread-only GPU vector renderer. No screen-sized CPU bitmap or readback.
use super::native_preview_backend_windows::{
    create_composition_swap_chain, create_d3d_device, create_direct_composition_tree,
    resolve_dxgi_factory,
};
use crate::error::{AppError, AppResult};
use windows::{
    core::Interface,
    Win32::{
        Foundation::HWND,
        Graphics::{
            Direct2D::{
                Common::{
                    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F,
                },
                D2D1CreateFactory, ID2D1DeviceContext, ID2D1Factory1, ID2D1Image,
                ID2D1SolidColorBrush, ID2D1StrokeStyle, D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1,
                D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_ELLIPSE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
            },
            DirectComposition::{IDCompositionDevice, IDCompositionTarget, IDCompositionVisual},
            Dxgi::{
                Common::DXGI_FORMAT_B8G8R8A8_UNORM, IDXGIDevice, IDXGIDevice1, IDXGISurface,
                IDXGISwapChain1, DXGI_PRESENT, DXGI_SWAP_CHAIN_FLAG,
            },
        },
    },
};

#[derive(Clone, Debug)]
pub(super) enum Primitive {
    Fill([f32; 4], [u8; 4]),
    Rect([f32; 4], f32, [u8; 4]),
    Ellipse([f32; 4], Option<f32>, [u8; 4]),
    Line([f32; 4], f32, [u8; 4]),
}

pub(super) struct SelectionRenderer {
    context: ID2D1DeviceContext,
    brush: ID2D1SolidColorBrush,
    swap_chain: IDXGISwapChain1,
    composition: IDCompositionDevice,
    _target: IDCompositionTarget,
    _visual: IDCompositionVisual,
    size: (u32, u32),
}

impl SelectionRenderer {
    pub fn new(hwnd: isize) -> AppResult<Self> {
        // Reuse the same hardware/WARP fallback as the existing native preview.
        let (device, _) = create_d3d_device()?;
        let dxgi: IDXGIDevice = device.cast().map_err(render_error)?;
        let latency: IDXGIDevice1 = dxgi.cast().map_err(render_error)?;
        unsafe { latency.SetMaximumFrameLatency(1) }.map_err(render_error)?;
        let factory = resolve_dxgi_factory(&dxgi)?;
        let swap_chain = create_composition_swap_chain(&factory, &device)?;
        let (composition, target, visual) =
            create_direct_composition_tree(&dxgi, HWND(hwnd as *mut _), &swap_chain)?;
        let factory: ID2D1Factory1 =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None) }
                .map_err(render_error)?;
        let d2d_device = unsafe { factory.CreateDevice(&dxgi) }.map_err(render_error)?;
        let context = unsafe { d2d_device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE) }
            .map_err(render_error)?;
        unsafe { context.SetDpi(96.0, 96.0) }; // All geometry is already in physical pixels.
        let brush = unsafe { context.CreateSolidColorBrush(&D2D1_COLOR_F::default(), None) }
            .map_err(render_error)?;
        let mut renderer = Self {
            context,
            brush,
            swap_chain,
            composition,
            _target: target,
            _visual: visual,
            size: (1, 1),
        };
        renderer.bind_target()?;
        Ok(renderer)
    }

    fn bind_target(&mut self) -> AppResult<()> {
        let surface: IDXGISurface =
            unsafe { self.swap_chain.GetBuffer(0) }.map_err(render_error)?;
        let properties = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
            ..Default::default()
        };
        let bitmap = unsafe {
            self.context
                .CreateBitmapFromDxgiSurface(&surface, Some(&properties))
        }
        .map_err(render_error)?;
        unsafe { self.context.SetTarget(&bitmap) };
        Ok(())
    }

    pub fn render(&mut self, width: u32, height: u32, primitives: &[Primitive]) -> AppResult<bool> {
        let resized = self.size != (width, height);
        if resized {
            unsafe { self.context.SetTarget(None::<&ID2D1Image>) };
            unsafe {
                self.swap_chain.ResizeBuffers(
                    2,
                    width,
                    height,
                    DXGI_FORMAT_B8G8R8A8_UNORM,
                    DXGI_SWAP_CHAIN_FLAG(0),
                )
            }
            .map_err(render_error)?;
            self.size = (width, height);
            self.bind_target()?;
        }
        unsafe {
            self.context.BeginDraw();
            self.context.Clear(Some(&D2D1_COLOR_F::default()));
            for primitive in primitives {
                let (Primitive::Fill(bounds, color)
                | Primitive::Rect(bounds, _, color)
                | Primitive::Ellipse(bounds, _, color)
                | Primitive::Line(bounds, _, color)) = primitive;
                self.brush.SetColor(&D2D1_COLOR_F {
                    r: f32::from(color[2]) / 255.0,
                    g: f32::from(color[1]) / 255.0,
                    b: f32::from(color[0]) / 255.0,
                    a: f32::from(color[3]) / 255.0,
                });
                let rect = D2D_RECT_F {
                    left: bounds[0],
                    top: bounds[1],
                    right: bounds[2],
                    bottom: bounds[3],
                };
                match primitive {
                    Primitive::Fill(..) => self.context.FillRectangle(&rect, &self.brush),
                    Primitive::Rect(_, stroke, _) => self.context.DrawRectangle(
                        &rect,
                        &self.brush,
                        *stroke,
                        None::<&ID2D1StrokeStyle>,
                    ),
                    Primitive::Ellipse(_, stroke, _) => {
                        let mut ellipse = D2D1_ELLIPSE::default();
                        ellipse.point.X = (rect.left + rect.right) * 0.5;
                        ellipse.point.Y = (rect.top + rect.bottom) * 0.5;
                        ellipse.radiusX = (rect.right - rect.left) * 0.5;
                        ellipse.radiusY = (rect.bottom - rect.top) * 0.5;
                        if let Some(stroke) = stroke {
                            self.context.DrawEllipse(
                                &ellipse,
                                &self.brush,
                                *stroke,
                                None::<&ID2D1StrokeStyle>,
                            );
                        } else {
                            self.context.FillEllipse(&ellipse, &self.brush);
                        }
                    }
                    Primitive::Line(_, stroke, _) => {
                        let mut start = D2D1_ELLIPSE::default().point;
                        start.X = rect.left;
                        start.Y = rect.top;
                        let mut end = start;
                        end.X = rect.right;
                        end.Y = rect.bottom;
                        self.context.DrawLine(
                            start,
                            end,
                            &self.brush,
                            *stroke,
                            None::<&ID2D1StrokeStyle>,
                        );
                    }
                }
            }
            self.context.EndDraw(None, None).map_err(render_error)?;
            // Vsync + one queued frame: no busy repaint loop and no growing frame backlog.
            self.swap_chain
                .Present(1, DXGI_PRESENT(0))
                .ok()
                .map_err(render_error)?;
            self.composition.Commit().map_err(render_error)?;
        }
        Ok(resized)
    }
}

fn render_error(error: windows::core::Error) -> AppError {
    AppError::new(
        "NATIVE_INTERACTION_GPU_RENDER_FAILED",
        "原生截图选区 GPU 绘制失败",
    )
    .with_detail("reason", error.to_string())
}
