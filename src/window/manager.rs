use std::{collections::HashMap, num::NonZero, sync::mpsc::Sender, thread};

use anyhow::{Context, anyhow};
use channel_protocol::channel_protocol;
use log::{debug, info};
use windows::Win32::{
    Foundation::RECT,
    Graphics::Dwm::{
        DWM_THUMBNAIL_PROPERTIES, DWM_TNP_RECTDESTINATION, DWM_TNP_VISIBLE, DwmRegisterThumbnail,
        DwmUnregisterThumbnail, DwmUpdateThumbnailProperties,
    },
    UI::WindowsAndMessaging::{
        GWL_EXSTYLE, SW_HIDE, SWP_SHOWWINDOW, SetWindowLongPtrW, SetWindowPos, ShowWindow,
        WS_EX_NOACTIVATE,
    },
};
use winit::{
    application::ApplicationHandler,
    dpi::{PhysicalPosition, PhysicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, OwnedDisplayHandle},
    platform::windows::EventLoopBuilderExtWindows,
    window::WindowId,
};

use crate::{
    Event, try_cast,
    utils::{Position, Size, cast::FaillibleCastUtils, color::Color},
    wincall_into_result, wincall_result,
    window::{
        Window,
        manager::utils::{WindowUtils, create_new_border_window},
    },
};

#[derive(Debug)]
pub struct BorderStyle {
    pub color: Color,
    pub thickness: u8,
    pub radius: u8,
}

pub type ThumbnailId = isize;

#[channel_protocol]
pub trait InputProtocol {
    fn create_thumbnail(src: Window, at: Position, size: Size) -> anyhow::Result<ThumbnailId>;
    fn close_all_thumbnails() -> anyhow::Result<()>;
    fn border_thumbnail(id: ThumbnailId) -> anyhow::Result<()>;
    fn unborder_all_thumbnails() -> anyhow::Result<()>;
    fn border_tiler_window(window: Window) -> anyhow::Result<()>;
}

#[channel_protocol]
pub trait OutputProtocol {
    fn cursor_entered_thumbnail(id: ThumbnailId);
    fn cursor_exited_thumbnail(id: ThumbnailId);
    fn thumbnail_clicked(id: ThumbnailId);

    fn unrecoverable_error(err: anyhow::Error);
}

struct Thumbnail {
    id: isize,
    window: winit::window::Window,
}

struct Border(winit::window::Window);

impl Border {
    pub const fn window(&self) -> &winit::window::Window {
        &self.0
    }
}

pub struct App {
    thumbnails: HashMap<ThumbnailId, Thumbnail>,
    thumbnail_border_style: BorderStyle,
    thumbnail_border: Option<Border>,

    tiler_border_style: BorderStyle,
    tiler_border: Option<Border>,

    context: softbuffer::Context<OwnedDisplayHandle>,
    output_client: OutputProtocolClient,
}

impl HandleInputProtocolWithState<&ActiveEventLoop> for App {
    fn create_thumbnail(
        &mut self,
        src: Window,
        at: Position,
        size: Size,
        event_loop: &ActiveEventLoop,
    ) -> anyhow::Result<ThumbnailId> {
        let window = utils::create_window(
            event_loop,
            winit::window::WindowAttributes::default()
                .with_title("thumbnail")
                .with_active(false)
                .with_position(PhysicalPosition::new(at.x(), at.y()))
                .with_inner_size(PhysicalSize::new(size.width(), size.height()))
                .with_visible(false)
                .with_decorations(false),
        )?;

        let thumbnail_hwnd = window.hwnd()?;

        wincall_into_result!(SetWindowLongPtrW(
            thumbnail_hwnd,
            GWL_EXSTYLE,
            WS_EX_NOACTIVATE.0.try_cast()?
        ))?;

        let thumbnail_id = wincall_result!(DwmRegisterThumbnail(thumbnail_hwnd, src.handle()))?;

        let thumbnail_props = DWM_THUMBNAIL_PROPERTIES {
            dwFlags: DWM_TNP_RECTDESTINATION | DWM_TNP_VISIBLE,
            rcDestination: RECT {
                left: 0,
                top: 0,
                right: size.width().try_cast()?,
                bottom: size.height().try_cast()?,
            },
            rcSource: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            opacity: 0,
            fVisible: true.into(),
            fSourceClientAreaOnly: true.into(),
        };

        wincall_result!(DwmUpdateThumbnailProperties(
            thumbnail_id,
            &raw const thumbnail_props
        ))?;

        {
            let win = window.to_crate_window()?;
            win.show()?;
            win.set_max_zindex()?;
        }
        window.set_visible(true);

        self.thumbnails.insert(
            thumbnail_id,
            Thumbnail {
                window,
                id: thumbnail_id,
            },
        );

        Ok(thumbnail_id)
    }

    fn close_all_thumbnails(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        self.unborder_all_thumbnails(event_loop)?;

        for Thumbnail { id, .. } in self.thumbnails.values() {
            wincall_result!(DwmUnregisterThumbnail(*id))?;
        }

        self.thumbnails.clear();
        Ok(())
    }

    fn border_thumbnail(&mut self, id: ThumbnailId, _: &ActiveEventLoop) -> anyhow::Result<()> {
        let thumbnail = self.thumbnails.get(&id).context(id)?;
        let border = self.get_initialized_thumbnail_border()?;

        self.border_window(
            thumbnail.window.to_crate_window()?,
            border,
            &self.thumbnail_border_style,
        )?;

        Ok(())
    }

    fn unborder_all_thumbnails(&mut self, _: &ActiveEventLoop) -> anyhow::Result<()> {
        let window = self.get_initialized_thumbnail_border()?.window();

        window.set_visible(false); // winit set_visible doesn't work. So we use ShowWindow directly. But to maintain state consistency inside winit we also call set_visible.
        let _ = wincall_into_result!(ShowWindow(window.hwnd()?, SW_HIDE))?;

        Ok(())
    }

    fn border_tiler_window(
        &mut self,
        window: Window,
        _state: &ActiveEventLoop,
    ) -> anyhow::Result<()> {
        let border_window = self.get_initialized_tiler_border()?;

        self.border_window(window, border_window, &self.tiler_border_style)?;

        Ok(())
    }
}

impl App {
    fn find_thumbnail_by_window_id(&self, window_id: WindowId) -> Option<&Thumbnail> {
        self.thumbnails
            .iter()
            .find(|(_, thumbnail)| thumbnail.window.id() == window_id)
            .map(|(_, thumbnail)| thumbnail)
    }

    fn get_initialized_thumbnail_border(&self) -> anyhow::Result<&Border> {
        let border_window = self
            .thumbnail_border
            .as_ref()
            .context("Uninitialized thumbnail border")?;
        Ok(border_window)
    }

    fn get_initialized_tiler_border(&self) -> anyhow::Result<&Border> {
        let border_window = self
            .tiler_border
            .as_ref()
            .context("Uninitialized tiler border")?;
        Ok(border_window)
    }

    fn handle_window_event(&self, event: &WindowEvent, window_id: WindowId) {
        let Some(thumbnail) = self.find_thumbnail_by_window_id(window_id) else {
            return;
        };

        match event {
            WindowEvent::CursorEntered { .. } => {
                self.output_client.cursor_entered_thumbnail(thumbnail.id);
            }
            WindowEvent::CursorLeft { .. } => {
                self.output_client.cursor_exited_thumbnail(thumbnail.id);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if *state == winit::event::ElementState::Pressed
                    && *button == winit::event::MouseButton::Left
                {
                    self.output_client.thumbnail_clicked(thumbnail.id);
                }
            }
            _ => {}
        }
    }

    fn border_window(
        &self,
        dest: Window,
        border: &Border,
        border_style: &BorderStyle,
    ) -> anyhow::Result<()> {
        let dest_bounds = dest.desktop_manager_bounds()?;
        let Position([dest_x, dest_y]) = dest_bounds.position();
        let Size([dest_width, dest_height]) = dest_bounds.size();

        try_cast! {
            border_style.thickness => i32 as thickness,
            dest_width => i32,
            dest_height => i32,
        }

        let border_window_x = dest_x - thickness;
        let border_window_y = dest_y - thickness;
        let border_window_width = dest_width + thickness * 2;
        let border_window_height = dest_height + thickness * 2;

        let border_window = border.window().to_crate_window()?;

        border_window.move_to(
            [border_window_x, border_window_y].into(),
            [
                border_window_width.try_cast()?,
                border_window_height.try_cast()?,
            ]
            .into(),
        )?;

        wincall_result!(SetWindowPos(
            border_window.handle(),
            Some(dest.handle()),
            border_window_x,
            border_window_y,
            border_window_width,
            border_window_height,
            SWP_SHOWWINDOW,
        ))?;

        border.window().set_visible(true);

        border_window.set_max_zindex()?;
        dest.set_max_zindex()?;

        self.prepare_border_surface(border, border_style)?;

        Ok(())
    }

    fn prepare_border_surface(
        &self,
        border: &Border,
        border_style: &BorderStyle,
    ) -> anyhow::Result<()> {
        let border_window = border.window();

        let mut surface = softbuffer::Surface::new(&self.context, &border_window)
            .map_err(|e| anyhow!("{e:?}"))
            .context("Softbuffer surface creation for border window")?;

        surface
            .resize(
                NonZero::new(border_window.inner_size().width).context("border window width")?,
                NonZero::new(border_window.inner_size().height).context("border window height")?,
            )
            .map_err(|e| anyhow!("{e:?}"))
            .context("Softbuffer surface resize to match border window")?;

        let mut buffer = surface
            .buffer_mut()
            .map_err(|e| anyhow!("{e:?}"))
            .context("border window surface mutable buffer extraction")?;

        buffer.fill(0);

        let border_pixmap = utils::draw_border(
            border
                .window()
                .to_crate_window()?
                .desktop_manager_bounds()?
                .size(),
            border_style,
        );

        for (out, [r, g, b, a]) in buffer
            .iter_mut()
            .zip(border_pixmap.data().as_chunks().0.iter())
        {
            let color = u32::from_be_bytes([*a, *r, *g, *b]);
            *out = color;
        }

        buffer
            .present()
            .map_err(|e| anyhow!("{e:?}"))
            .context("Border window buffer presentation")?;

        Ok(())
    }

    fn initialize_thumbnail_border_window(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> anyhow::Result<()> {
        let border_window = create_new_border_window(event_loop)?;
        self.thumbnail_border = Some(Border(border_window));
        Ok(())
    }

    fn create_tiler_border_window(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        let border_window = create_new_border_window(event_loop)?;
        self.tiler_border = Some(Border(border_window));
        Ok(())
    }
}

impl ApplicationHandler<InputProtocolMessage> for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        info!("Window manager started");
        if let Err(e) = self
            .initialize_thumbnail_border_window(event_loop)
            .and_then(|()| self.create_tiler_border_window(event_loop))
        {
            self.output_client.unrecoverable_error(e);
        }
    }

    fn user_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        message: InputProtocolMessage,
    ) {
        debug!("Window manager recieved event {message:#?}");
        self.dispatch_with_state(message, event_loop);
    }

    fn window_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: WindowId,
        event: winit::event::WindowEvent,
    ) {
        self.handle_window_event(&event, window_id);
    }
}

pub fn launch(
    event_tx: Sender<Event>,
    thumbnail_border_style: BorderStyle,
    tiler_border_style: BorderStyle,
) -> anyhow::Result<InputProtocolClient> {
    let (input_client, input_rx) = InputProtocolClient::new();
    let (output_client, output_rx) = OutputProtocolClient::new();

    thread::Builder::new()
        .name("Window manager".into())
        .spawn(move || {
            let event_loop = EventLoop::with_user_event()
                .with_any_thread(true)
                .build()
                .expect("Window manager event loop");

            let event_sender = event_loop.create_proxy();

            thread::spawn(move || {
                for input_event in input_rx {
                    event_sender.send_event(input_event).unwrap();
                }
            });

            let context = softbuffer::Context::new(event_loop.owned_display_handle())
                .map_err(|e| anyhow!("{e:?}"))
                .context("Softbuffer context creation for window manager");

            let context = match context {
                Ok(c) => c,
                Err(err) => {
                    output_client.unrecoverable_error(err);
                    return;
                }
            };

            event_loop
                .run_app(&mut App {
                    thumbnails: HashMap::default(),
                    thumbnail_border_style,
                    thumbnail_border: None,
                    tiler_border_style,
                    tiler_border: None,
                    context,
                    output_client,
                })
                .expect("Window manager execution");
        })?;

    thread::Builder::new()
        .name("window manager output event mapper".into())
        .spawn(move || {
            for output_event in output_rx {
                event_tx.send(Event::WindowManager(output_event)).unwrap();
            }
        })?;

    Ok(input_client)
}

pub mod utils {
    use anyhow::bail;
    use raw_window_handle::HasWindowHandle;
    use tiny_skia::{Pixmap, Stroke};
    use windows::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GWL_STYLE, SetWindowLongPtrW, WS_EX_NOACTIVATE, WS_POPUP,
        },
    };
    use winit::{
        event_loop::ActiveEventLoop, platform::windows::WindowAttributesExtWindows,
        window::WindowAttributes,
    };

    use crate::{
        utils::{Size, cast::FaillibleCastUtils},
        wincall_into_result,
        window::manager::BorderStyle,
    };

    pub const WINRI_WINDOW_MANAGER_CLASS_NAME: &str = "WinriWindowManagerWindow";

    #[easy_ext::ext(WindowUtils)]
    impl winit::window::Window {
        pub fn hwnd(&self) -> anyhow::Result<HWND> {
            let handle = self.window_handle()?;
            let handle = match handle.as_raw() {
                raw_window_handle::RawWindowHandle::Win32(win32_window_handle) => {
                    win32_window_handle.hwnd
                }
                _ => bail!("Unsupported platform"),
            };
            let handle = handle.get() as *mut std::ffi::c_void;
            Ok(HWND(handle))
        }

        pub fn to_crate_window(&self) -> anyhow::Result<crate::window::Window> {
            crate::window::Window::from_hwnd(self.hwnd()?)
        }
    }

    pub fn create_window(
        event_loop: &ActiveEventLoop,
        attrib: WindowAttributes,
    ) -> anyhow::Result<winit::window::Window> {
        let attrib = attrib.with_class_name(WINRI_WINDOW_MANAGER_CLASS_NAME);
        let create_window = event_loop.create_window(attrib)?;
        Ok(create_window)
    }

    pub fn create_new_border_window(
        event_loop: &ActiveEventLoop,
    ) -> anyhow::Result<winit::window::Window> {
        let border_window = create_window(
            event_loop,
            WindowAttributes::default()
                .with_visible(false)
                .with_decorations(false)
                .with_transparent(true),
        )?;

        let hwnd = border_window.hwnd()?;

        wincall_into_result!(SetWindowLongPtrW(hwnd, GWL_STYLE, WS_POPUP.0.try_cast()?))?;

        wincall_into_result!(SetWindowLongPtrW(
            hwnd,
            GWL_EXSTYLE,
            WS_EX_NOACTIVATE.0.try_cast()?
        ))?;

        Ok(border_window)
    }

    pub fn draw_border(border_window_size: Size, border_style: &BorderStyle) -> Pixmap {
        let mut pixmap = Pixmap::new(border_window_size.width(), border_window_size.height())
            .expect("Failed to create border pixmap");

        let border_color = tiny_skia::Color::from_rgba8(
            border_style.color.r,
            border_style.color.g,
            border_style.color.b,
            border_style.color.a,
        );

        let mut paint = tiny_skia::Paint::default();
        paint.set_color(border_color);
        paint.anti_alias = true;

        let stroke = Stroke {
            width: (border_style.thickness + 1).cast(),
            ..Default::default()
        };

        #[allow(clippy::cast_precision_loss)]
        let w = pixmap.width() as f32;
        #[allow(clippy::cast_precision_loss)]
        let h = pixmap.height() as f32;

        let border_radius = border_style.radius.cast();

        let mut path = tiny_skia::PathBuilder::new();

        let half_stroke_width = stroke.width / 2.0;

        // top edge
        path.move_to(border_radius, half_stroke_width);
        path.line_to(w - border_radius, half_stroke_width);
        path.cubic_to(
            w - half_stroke_width,
            half_stroke_width,
            w - half_stroke_width,
            border_radius,
            w - half_stroke_width,
            border_radius,
        );

        // right edge
        path.line_to(w - half_stroke_width, h - border_radius);

        path.cubic_to(
            w - half_stroke_width,
            h - half_stroke_width,
            w - border_radius,
            h - half_stroke_width,
            w - border_radius,
            h - half_stroke_width,
        );

        // bottom edge
        path.line_to(border_radius, h - half_stroke_width);
        path.cubic_to(
            half_stroke_width,
            h - half_stroke_width,
            half_stroke_width,
            h - border_radius,
            half_stroke_width,
            h - border_radius,
        );

        // left edge
        path.line_to(half_stroke_width, border_radius);
        path.cubic_to(
            half_stroke_width,
            half_stroke_width,
            border_radius,
            half_stroke_width,
            border_radius,
            half_stroke_width,
        );
        let path = path.finish().expect("Failed to create border path");

        pixmap.stroke_path(
            &path,
            &paint,
            &stroke,
            tiny_skia::Transform::identity(),
            None,
        );

        pixmap
    }
}
