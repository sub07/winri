use std::{
    num::NonZero,
    sync::mpsc::{self, Sender},
    thread,
};

use anyhow::{Context, anyhow, bail};
use log::{debug, info};
use raw_window_handle::HasWindowHandle;
use windows::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Dwm::{
        DWM_THUMBNAIL_PROPERTIES, DWM_TNP_RECTDESTINATION, DWM_TNP_VISIBLE, DwmRegisterThumbnail,
        DwmUnregisterThumbnail, DwmUpdateThumbnailProperties,
    },
    UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GWL_STYLE, SWP_SHOWWINDOW, SetWindowLongPtrW, SetWindowPos, WS_EX_NOACTIVATE,
        WS_POPUP,
    },
};
use winit::{
    application::ApplicationHandler,
    dpi::{PhysicalPosition, PhysicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    platform::windows::{EventLoopBuilderExtWindows, WindowAttributesExtWindows},
    window::{WindowAttributes, WindowId},
};

use crate::{
    Event, try_cast,
    utils::{CastUtils, color::Color},
    wincall_into_result, wincall_result,
    window::Window,
};

#[derive(Debug)]
struct CreateThumbnailEvent {
    src: Window,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    dest_tx: oneshot::Sender<Window>,
}

#[derive(Debug)]
struct CreateThumbnailBorderEvent {
    thumbnail: ThumbnailDescriptor,
    thickness: u32,
    color: Color,
}

#[derive(Debug)]
struct CloseThumbnailBorderEvent {
    thumbnail: ThumbnailDescriptor,
}

#[derive(Debug)]
enum ThumbnailManagerEvent {
    CreateThumbnail(CreateThumbnailEvent),
    CloseAllThumbnails,
    CreateThumbnailBorder(CreateThumbnailBorderEvent),
    CloseThumbnailBorder(CloseThumbnailBorderEvent),
}

#[derive(Debug, Clone, Copy)]
pub struct ThumbnailDescriptor {
    pub src: Window,
    pub dest: Window,
    thumbnail_id: isize,
}

pub enum OutgoingEvent {
    CursorEnteredThumbnail(ThumbnailDescriptor),
    CursorLeftThumbnail(ThumbnailDescriptor),
    ManagerPanic(anyhow::Error),
}

type ThumbnailManagerEventSender = EventLoopProxy<ThumbnailManagerEvent>;

pub struct ThumbnailManager {
    tx: ThumbnailManagerEventSender,
}

impl ThumbnailManager {
    const fn new(tx: ThumbnailManagerEventSender) -> Self {
        Self { tx }
    }

    pub fn create_thumbnail(
        &self,
        src: Window,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> anyhow::Result<Window> {
        debug!("Requesting thumbnail creation for window {src:?}");
        let (dest_tx, dest_rx) = oneshot::channel();
        self.tx
            .send_event(ThumbnailManagerEvent::CreateThumbnail(
                CreateThumbnailEvent {
                    src,
                    x,
                    y,
                    width,
                    height,
                    dest_tx,
                },
            ))
            .context("Could not send CreateThumbnail event to ThumbnailManager")?;

        dest_rx
            .recv()
            .context("Could not receive thumbnail window from ThumbnailManager")
    }

    pub fn close_all_thumbnails(&self) -> anyhow::Result<()> {
        debug!("Requesting all thumbnails to be closed");
        self.tx
            .send_event(ThumbnailManagerEvent::CloseAllThumbnails)
            .context("Could not send CloseAllThumbnails event to ThumbnailManager")?;
        Ok(())
    }

    pub fn display_border(
        &self,
        thumbnail: ThumbnailDescriptor,
        thickness: u32,
        color: Color,
    ) -> anyhow::Result<()> {
        debug!("Requesting border display for thumbnail {thumbnail:?}");
        self.tx
            .send_event(ThumbnailManagerEvent::CreateThumbnailBorder(
                CreateThumbnailBorderEvent {
                    thumbnail,
                    thickness,
                    color,
                },
            ))
            .context("Could not send DisplayBorder event to ThumbnailManager")?;
        Ok(())
    }

    pub fn hide_border(&self, thumbnail: ThumbnailDescriptor) -> anyhow::Result<()> {
        debug!("Requesting border hide for thumbnail {thumbnail:?}");
        self.tx
            .send_event(ThumbnailManagerEvent::CloseThumbnailBorder(
                CloseThumbnailBorderEvent { thumbnail },
            ))
            .context("Could not send HideBorder event to ThumbnailManager")?;
        Ok(())
    }
}

struct Thumbnail {
    src: Window,
    dest: winit::window::Window,
    id: isize,
    border: Option<winit::window::Window>,
}

impl Thumbnail {
    fn to_descriptor(&self) -> anyhow::Result<ThumbnailDescriptor> {
        Ok(ThumbnailDescriptor {
            src: self.src,
            dest: Window::from_hwnd(self.dest.handle()?)?,
            thumbnail_id: self.id,
        })
    }
}

struct ThumbnailManagerApp {
    thumbnails: Vec<Thumbnail>,
    outgoing_tx: mpsc::Sender<OutgoingEvent>,
}

impl ThumbnailManagerApp {
    fn find_thumbnail_mut(&mut self, descriptor: ThumbnailDescriptor) -> Option<&mut Thumbnail> {
        self.thumbnails
            .iter_mut()
            .find(|thumbnail| thumbnail.id == descriptor.thumbnail_id)
    }

    fn find_thumbnail_by_window_id(&self, window_id: WindowId) -> Option<&Thumbnail> {
        self.thumbnails
            .iter()
            .find(|thumbnail| thumbnail.dest.id() == window_id)
    }

    fn handle_window_event(&self, event: &WindowEvent, window_id: WindowId) -> anyhow::Result<()> {
        let Some(thumbnail) = self.find_thumbnail_by_window_id(window_id) else {
            return Ok(());
        };

        match event {
            WindowEvent::CursorEntered { .. } => {
                self.outgoing_tx
                    .send(OutgoingEvent::CursorEnteredThumbnail(
                        thumbnail.to_descriptor()?,
                    ))?;
            }
            WindowEvent::CursorLeft { .. } => self.outgoing_tx.send(
                OutgoingEvent::CursorLeftThumbnail(thumbnail.to_descriptor()?),
            )?,
            _ => {}
        }

        Ok(())
    }

    fn handle_create_thumbnail_event(
        &mut self,
        CreateThumbnailEvent {
            src,
            x,
            y,
            width,
            height,
            dest_tx,
        }: CreateThumbnailEvent,
        event_loop: &ActiveEventLoop,
    ) -> anyhow::Result<()> {
        let window = event_loop.create_window(
            winit::window::WindowAttributes::default()
                .with_title("thumbnail")
                .with_active(false)
                .with_inner_size(PhysicalSize::new(width, height))
                .with_position(PhysicalPosition::new(x, y))
                .with_system_backdrop(winit::platform::windows::BackdropType::None)
                .with_decorations(false),
        )?;

        let dest_handle = window.handle()?;

        wincall_into_result!(SetWindowLongPtrW(
            dest_handle,
            GWL_EXSTYLE,
            WS_EX_NOACTIVATE.0.try_cast()?
        ))?;

        let thumbnail_id = wincall_result!(DwmRegisterThumbnail(dest_handle, src.handle()))?;

        let thumbnail_props = DWM_THUMBNAIL_PROPERTIES {
            dwFlags: DWM_TNP_RECTDESTINATION | DWM_TNP_VISIBLE,
            rcDestination: RECT {
                left: 0,
                top: 0,
                right: width.try_cast()?,
                bottom: height.try_cast()?,
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

        dest_tx.send(Window::from_hwnd(window.handle()?)?)?;

        self.thumbnails.push(Thumbnail {
            src,
            dest: window,
            id: thumbnail_id,
            border: None,
        });

        Ok(())
    }

    fn handle_close_all_thumbnails_event(&mut self) -> anyhow::Result<()> {
        for thumbnail in &self.thumbnails {
            wincall_result!(DwmUnregisterThumbnail(thumbnail.id))?;
        }
        self.thumbnails.clear();
        Ok(())
    }

    fn handle_create_thumbnail_border_event(
        &mut self,
        CreateThumbnailBorderEvent {
            thumbnail,
            thickness,
            color,
        }: CreateThumbnailBorderEvent,
        event_loop: &ActiveEventLoop,
    ) -> anyhow::Result<()> {
        let thumbnail = self.find_thumbnail_mut(thumbnail).ok_or_else(|| {
            anyhow!("Could not find thumbnail {thumbnail:?} to create border for")
        })?;

        let border_window = event_loop
            .create_window(
                WindowAttributes::default()
                    .with_visible(false)
                    .with_transparent(true)
                    .with_decorations(false),
            )
            .context("Border window creation")?;

        let dest_position = thumbnail.dest.outer_position()?;
        let dest_size = thumbnail.dest.outer_size();

        try_cast! {
            thickness => i32,
            (dest_size.width) => i32 as dest_width,
            (dest_size.height) => i32 as dest_height,
        }

        let border_window_x = dest_position.x - thickness;
        let border_window_y = dest_position.y - thickness;
        let border_window_width: i32 = dest_width + thickness * 2;
        let border_window_height: i32 = dest_height + thickness * 2;

        let border_window_handle = border_window.handle()?;

        wincall_into_result!(SetWindowLongPtrW(
            border_window_handle,
            GWL_STYLE,
            WS_POPUP.0.try_cast()?
        ))?;

        wincall_into_result!(SetWindowLongPtrW(
            border_window_handle,
            GWL_EXSTYLE,
            WS_EX_NOACTIVATE.0.try_cast()?
        ))?;

        border_window.set_visible(true);
        wincall_result!(SetWindowPos(
            border_window_handle,
            Some(thumbnail.dest.handle()?),
            border_window_x,
            border_window_y,
            border_window_width,
            border_window_height,
            SWP_SHOWWINDOW,
        ))?;

        {
            let context = softbuffer::Context::new(event_loop.owned_display_handle())
                .map_err(|e| anyhow!("{e:?}"))
                .context("Softbuffer context creation for border window")?;
            let mut surface = softbuffer::Surface::new(&context, &border_window)
                .map_err(|e| anyhow!("{e:?}"))
                .context("Softbuffer surface creation for border window")?;
            surface
                .resize(
                    NonZero::new(border_window.inner_size().width)
                        .context("border window width")?,
                    NonZero::new(border_window.inner_size().height)
                        .context("border window height")?,
                )
                .map_err(|e| anyhow!("{e:?}"))
                .context("Softbuffer surface resize to match border window")?;
            let mut buffer = surface
                .buffer_mut()
                .map_err(|e| anyhow!("{e:?}"))
                .context("border window surface mutable buffer extraction")?;
            for pixel in buffer.iter_mut() {
                *pixel = color.into_argb_packed();
            }
            buffer
                .present()
                .map_err(|e| anyhow!("{e:?}"))
                .context("Border window buffer presentation")?;
        }

        thumbnail.border = Some(border_window);

        Ok(())
    }

    fn handle_close_thumbnail_border_event(
        &mut self,
        CloseThumbnailBorderEvent { thumbnail }: CloseThumbnailBorderEvent,
    ) -> anyhow::Result<()> {
        let thumbnail = self
            .find_thumbnail_mut(thumbnail)
            .ok_or_else(|| anyhow!("Could not find thumbnail {thumbnail:?} to close border for"))?;

        thumbnail.border = None;

        Ok(())
    }

    fn handle_manager_event(
        &mut self,
        event: ThumbnailManagerEvent,
        event_loop: &ActiveEventLoop,
    ) -> anyhow::Result<()> {
        match event {
            ThumbnailManagerEvent::CreateThumbnail(event) => {
                self.handle_create_thumbnail_event(event, event_loop)?;
            }
            ThumbnailManagerEvent::CloseAllThumbnails => {
                self.handle_close_all_thumbnails_event()?;
            }
            ThumbnailManagerEvent::CreateThumbnailBorder(event) => {
                self.handle_create_thumbnail_border_event(event, event_loop)?;
            }
            ThumbnailManagerEvent::CloseThumbnailBorder(event) => {
                self.handle_close_thumbnail_border_event(event)?;
            }
        }
        Ok(())
    }
}

impl ApplicationHandler<ThumbnailManagerEvent> for ThumbnailManagerApp {
    fn resumed(&mut self, _: &winit::event_loop::ActiveEventLoop) {
        info!("ThumbnailManager started");
    }

    fn user_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        event: ThumbnailManagerEvent,
    ) {
        if let Err(err) = self.handle_manager_event(event, event_loop) {
            self.outgoing_tx
                .send(OutgoingEvent::ManagerPanic(err))
                .unwrap();
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        if let Err(err) = self.handle_window_event(&event, window_id) {
            self.outgoing_tx
                .send(OutgoingEvent::ManagerPanic(err))
                .unwrap();
        }
    }
}

#[easy_ext::ext]
impl winit::window::Window {
    pub fn handle(&self) -> anyhow::Result<HWND> {
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
}

pub fn launch_thumbnail_manager(event_tx: Sender<Event>) -> ThumbnailManager {
    let (tx, rx) = oneshot::channel();
    let (outgoing_tx, outgoing_rx) = mpsc::channel();
    thread::Builder::new()
        .name("Thumbnail manager".into())
        .spawn(move || {
            let event_loop = EventLoop::with_user_event()
                .with_any_thread(true)
                .build()
                .expect("Thumbnail manager event loop");

            tx.send(event_loop.create_proxy())
                .expect("Could not send thumbnail manager event sender to main thread");

            info!("Starting ThumbnailManager event loop");
            event_loop
                .run_app(&mut ThumbnailManagerApp {
                    thumbnails: vec![],
                    outgoing_tx,
                })
                .expect("ThumbnailManager running");
        })
        .expect("Could not spawn thumbnail manager thread");

    let event_sender = rx
        .recv()
        .expect("Could not open communication with thumbnail manager thread");

    thread::Builder::new()
        .name("Thumbnail manager outgoing events".into())
        .spawn(move || {
            for outgoing_event in outgoing_rx {
                event_tx
                    .send(Event::Thumbnail(outgoing_event))
                    .expect("Could not send thumbnail outgoing event to main event channel");
            }
        })
        .expect("Could not spawn thumbnail manager outgoing events thread");

    ThumbnailManager::new(event_sender)
}
