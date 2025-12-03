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
        GWL_EXSTYLE, GWL_STYLE, SW_HIDE, SWP_SHOWWINDOW, SetWindowLongPtrW, SetWindowPos,
        ShowWindow, WS_EX_NOACTIVATE, WS_POPUP,
    },
};
use winit::{
    application::ApplicationHandler,
    dpi::{PhysicalPosition, PhysicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, OwnedDisplayHandle},
    platform::windows::EventLoopBuilderExtWindows,
    window::{WindowAttributes, WindowId},
};

use crate::{
    Event, try_cast,
    utils::{Position, Size, cast::FaillibleCastUtils, color::Color},
    wincall_into_result, wincall_result,
    window::{Window, manager::utils::WindowUtils},
};

#[derive(Debug)]
pub struct BorderStyle {
    pub color: Color,
    pub thickness: u32,
}

pub type ThumbnailId = isize;

#[channel_protocol]
pub trait InputProtocol {
    fn create_thumbnail(src: Window, at: Position, size: Size) -> anyhow::Result<ThumbnailId>;
    fn close_all_thumbnails() -> anyhow::Result<()>;
    fn border_thumbnail(id: ThumbnailId) -> anyhow::Result<()>;
    fn unborder_all_thumbnails() -> anyhow::Result<()>;
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

pub struct App {
    thumbnails: HashMap<ThumbnailId, Thumbnail>,
    thumbnail_border_style: BorderStyle,
    thumbnail_border: Option<Border>,

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
        let Border(border_window) = self
            .thumbnail_border
            .as_ref()
            .context("Uninitialized thumbnail border")?;

        let dest_position = thumbnail.window.outer_position()?;
        let dest_size = thumbnail.window.outer_size();

        try_cast! {
            self.thumbnail_border_style.thickness => i32 as thickness,
            dest_size.width => i32 as dest_width,
            dest_size.height => i32 as dest_height,
        }

        let border_window_x = dest_position.x - thickness;
        let border_window_y = dest_position.y - thickness;
        let border_window_width: i32 = dest_width + thickness * 2;
        let border_window_height: i32 = dest_height + thickness * 2;

        {
            let bwindow = border_window.to_crate_window()?;

            bwindow.move_to(
                [border_window_x, border_window_y].into(),
                [
                    border_window_width.try_cast()?,
                    border_window_height.try_cast()?,
                ]
                .into(),
            )?;

            wincall_result!(SetWindowPos(
                border_window.hwnd()?,
                Some(thumbnail.window.hwnd()?),
                border_window_x,
                border_window_y,
                border_window_width,
                border_window_height,
                SWP_SHOWWINDOW,
            ))?;

            border_window.set_visible(true);

            bwindow.set_max_zindex()?;
            thumbnail.window.to_crate_window()?.set_max_zindex()?;
        }

        self.prepare_border_window_for_thumbnail(border_window)?;

        Ok(())
    }

    fn unborder_all_thumbnails(&mut self, _: &ActiveEventLoop) -> anyhow::Result<()> {
        let window = self.get_initialized_border_window()?;

        window.set_visible(false); // winit set_visible doesn't work. So we use ShowWindow directly. But to maintain state consistency inside winit we also call set_visible.
        let _ = wincall_into_result!(ShowWindow(window.hwnd()?, SW_HIDE))?;

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

    fn get_initialized_border_window(&self) -> anyhow::Result<&winit::window::Window> {
        let border_window = self
            .thumbnail_border
            .as_ref()
            .context("Uninitialized thumbnail border")?;
        Ok(&border_window.0)
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

    fn create_thumbnail_border_window(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> anyhow::Result<()> {
        let border_window = utils::create_window(
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

        self.thumbnail_border = Some(Border(border_window));

        Ok(())
    }

    fn prepare_border_window_for_thumbnail(
        &self,
        border_window: &winit::window::Window,
    ) -> anyhow::Result<()> {
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

        buffer.fill(
            self.thumbnail_border_style
                .color
                .without_alpha()
                .into_argb_packed(),
        );

        buffer
            .present()
            .map_err(|e| anyhow!("{e:?}"))
            .context("Border window buffer presentation")?;

        Ok(())
    }
}

impl ApplicationHandler<InputProtocolMessage> for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        info!("Window manager started");
        if let Err(e) = self.create_thumbnail_border_window(event_loop) {
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
    use windows::Win32::Foundation::HWND;
    use winit::{
        event_loop::ActiveEventLoop, platform::windows::WindowAttributesExtWindows,
        window::WindowAttributes,
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
}
