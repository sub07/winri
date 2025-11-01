use std::{
    num::NonZero,
    sync::mpsc::{self, Sender},
    thread,
};

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
    event_loop::{EventLoop, EventLoopProxy},
    platform::windows::{EventLoopBuilderExtWindows, WindowAttributesExtWindows},
    window::WindowAttributes,
};

use crate::{
    Event, cast,
    utils::{CastUtils, color::Color},
    window::Window,
};

#[derive(Debug)]
enum ThumbnailManagerEvent {
    CreateThumbnail {
        src: Window,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        dest_tx: oneshot::Sender<Window>,
    },
    CloseAllThumbnails,
    DisplayBorder {
        thumbnail: ThumbnailDescriptor,
        thickness: u32,
        color: Color,
    },
    HideBorder {
        thumbnail: ThumbnailDescriptor,
    },
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
}

type ThumbnailManagerEventSender = EventLoopProxy<ThumbnailManagerEvent>;

pub struct ThumbnailManager {
    tx: ThumbnailManagerEventSender,
}

impl ThumbnailManager {
    const fn new(tx: ThumbnailManagerEventSender) -> Self {
        Self { tx }
    }

    pub fn create_thumbnail(&self, src: Window, x: i32, y: i32, width: u32, height: u32) -> Window {
        debug!("Requesting thumbnail creation for window {src:?}");
        let (dest_tx, dest_rx) = oneshot::channel();
        self.tx
            .send_event(ThumbnailManagerEvent::CreateThumbnail {
                src,
                x,
                y,
                width,
                height,
                dest_tx,
            })
            .expect("Could not send CreateThumbnail event to ThumbnailManager");

        dest_rx
            .recv()
            .expect("Could not receive thumbnail window from ThumbnailManager")
    }

    pub fn close_all_thumbnails(&self) {
        debug!("Requesting all thumbnails to be closed");
        self.tx
            .send_event(ThumbnailManagerEvent::CloseAllThumbnails)
            .expect("Could not send CloseAllThumbnails event to ThumbnailManager");
    }

    pub fn display_border(&self, thumbnail: ThumbnailDescriptor, thickness: u32, color: Color) {
        debug!("Requesting border display for thumbnail {thumbnail:?}");
        self.tx
            .send_event(ThumbnailManagerEvent::DisplayBorder {
                thumbnail,
                thickness,
                color,
            })
            .expect("Could not send DisplayBorder event to ThumbnailManager");
    }

    pub fn hide_border(&self, thumbnail: ThumbnailDescriptor) {
        debug!("Requesting border hide for thumbnail {thumbnail:?}");
        self.tx
            .send_event(ThumbnailManagerEvent::HideBorder { thumbnail })
            .expect("Could not send HideBorder event to ThumbnailManager");
    }
}

struct Thumbnail {
    src: Window,
    dest: winit::window::Window,
    id: isize,
    border: Option<winit::window::Window>,
}

impl Thumbnail {
    fn to_descriptor(&self) -> ThumbnailDescriptor {
        ThumbnailDescriptor {
            src: self.src,
            dest: Window::from_hwnd(self.dest.handle()).unwrap(),
            thumbnail_id: self.id,
        }
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
}

impl ApplicationHandler<ThumbnailManagerEvent> for ThumbnailManagerApp {
    fn resumed(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        info!("ThumbnailManager started");
    }
    #[allow(clippy::too_many_lines)]
    fn user_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        event: ThumbnailManagerEvent,
    ) {
        match event {
            ThumbnailManagerEvent::CreateThumbnail {
                src,
                x,
                y,
                width,
                height,
                dest_tx,
            } => {
                let window = event_loop
                    .create_window(
                        winit::window::WindowAttributes::default()
                            .with_title("thumbnail")
                            .with_active(false)
                            .with_inner_size(PhysicalSize::new(width, height))
                            .with_position(PhysicalPosition::new(x, y))
                            .with_system_backdrop(winit::platform::windows::BackdropType::None)
                            .with_decorations(false),
                    )
                    .unwrap();

                let dest_handle = window.handle();

                unsafe { SetWindowLongPtrW(dest_handle, GWL_EXSTYLE, WS_EX_NOACTIVATE.0.cast()) };

                let thumbnail_id =
                    unsafe { DwmRegisterThumbnail(dest_handle, src.handle()) }.unwrap();

                let thumbnail_props = DWM_THUMBNAIL_PROPERTIES {
                    dwFlags: DWM_TNP_RECTDESTINATION | DWM_TNP_VISIBLE,
                    rcDestination: RECT {
                        left: 0,
                        top: 0,
                        right: width.try_into().unwrap(),
                        bottom: height.try_into().unwrap(),
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

                unsafe {
                    DwmUpdateThumbnailProperties(thumbnail_id, &raw const thumbnail_props).unwrap();
                };

                dest_tx
                    .send(Window::from_hwnd(window.handle()).unwrap())
                    .expect("Could not send thumbnail window back to requester");

                self.thumbnails.push(Thumbnail {
                    src,
                    dest: window,
                    id: thumbnail_id,
                    border: None,
                });
            }
            ThumbnailManagerEvent::CloseAllThumbnails => {
                for thumbnail in &self.thumbnails {
                    unsafe {
                        DwmUnregisterThumbnail(thumbnail.id).unwrap();
                    }
                }
                self.thumbnails.clear();
            }
            ThumbnailManagerEvent::DisplayBorder {
                thumbnail,
                thickness,
                color,
            } => {
                let Some(thumbnail) = self.find_thumbnail_mut(thumbnail) else {
                    return;
                };

                let border_window = event_loop
                    .create_window(
                        WindowAttributes::default()
                            .with_visible(false)
                            .with_transparent(true)
                            .with_decorations(false),
                    )
                    .expect("Could not create border window");

                border_window.set_visible(true);
                border_window.set_visible(false);

                let dest_position = thumbnail.dest.outer_position().unwrap();
                let dest_size = thumbnail.dest.outer_size();

                cast! {
                    thickness => i32,
                    (dest_size.width) => i32 as dest_width,
                    (dest_size.height) => i32 as dest_height,
                }

                let border_window_x = dest_position.x - thickness;
                let border_window_y = dest_position.y - thickness;
                let border_window_width: i32 = dest_width + thickness * 2;
                let border_window_height: i32 = dest_height + thickness * 2;

                unsafe {
                    SetWindowLongPtrW(border_window.handle(), GWL_STYLE, WS_POPUP.0.cast());
                    SetWindowPos(
                        border_window.handle(),
                        Some(thumbnail.dest.handle()),
                        border_window_x,
                        border_window_y,
                        border_window_width,
                        border_window_height,
                        SWP_SHOWWINDOW,
                    )
                    .unwrap();
                };

                {
                    let context = softbuffer::Context::new(event_loop.owned_display_handle())
                        .expect("Could not create softbuffer context for border window");
                    let mut surface = softbuffer::Surface::new(&context, &border_window).unwrap();
                    surface
                        .resize(
                            NonZero::new(border_window.inner_size().width).unwrap(),
                            NonZero::new(border_window.inner_size().height).unwrap(),
                        )
                        .unwrap();
                    let mut buffer = surface.buffer_mut().unwrap();
                    for pixel in buffer.iter_mut() {
                        *pixel = color.into_argb_packed();
                    }
                    buffer.present().unwrap();
                }

                thumbnail.border = Some(border_window);
            }
            ThumbnailManagerEvent::HideBorder { thumbnail } => {
                let Some(thumbnail) = self.find_thumbnail_mut(thumbnail) else {
                    return;
                };

                thumbnail.border = None;
            }
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        let Some(thumbnail) = self
            .thumbnails
            .iter()
            .find(|thumbnail| thumbnail.dest.id() == window_id)
        else {
            return;
        };

        match event {
            WindowEvent::CursorEntered { .. } => {
                self.outgoing_tx
                    .send(OutgoingEvent::CursorEnteredThumbnail(
                        thumbnail.to_descriptor(),
                    ))
                    .unwrap();
            }
            WindowEvent::CursorLeft { .. } => {
                self.outgoing_tx
                    .send(OutgoingEvent::CursorLeftThumbnail(
                        thumbnail.to_descriptor(),
                    ))
                    .unwrap();
            }
            _ => {}
        }
    }
}

#[easy_ext::ext]
impl winit::window::Window {
    pub fn handle(&self) -> HWND {
        let handle = self.window_handle().unwrap();
        let handle = match handle.as_raw() {
            raw_window_handle::RawWindowHandle::Win32(win32_window_handle) => {
                win32_window_handle.hwnd
            }
            _ => unreachable!("Unsupported platform"),
        };
        let handle = handle.get() as *mut std::ffi::c_void;
        HWND(handle)
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
