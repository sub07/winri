use std::{
    ptr::null_mut,
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

use iced::futures::channel::mpsc::Sender;
use windows::Win32::UI::{
    Accessibility::{SetWinEventHook, UnhookWinEvent},
    WindowsAndMessaging::{
        EVENT_OBJECT_CREATE, EVENT_OBJECT_LOCATIONCHANGE, GetMessageW, WINEVENT_OUTOFCONTEXT,
        WINEVENT_SKIPOWNPROCESS,
    },
};

use crate::app::subscription::global::GlobalMessage;

const WINDOW_HOOK_COOLDOWN: Duration = Duration::from_millis(200);

struct WindowHookManager {
    tx: Sender<GlobalMessage>,
    last_time_notified: Instant,
}

impl WindowHookManager {
    fn new(notifier: Sender<GlobalMessage>) -> Self {
        Self {
            tx: notifier,
            last_time_notified: Instant::now(),
        }
    }

    fn tick(&mut self) {
        let elapsed = self.last_time_notified.elapsed();
        if elapsed > WINDOW_HOOK_COOLDOWN {
            self.tx.try_send(GlobalMessage::Window).unwrap();
            self.last_time_notified = Instant::now();
        } else {
            let original_last_time_notified = self.last_time_notified;
            // TODO: Needs serious rework to avoid spawning threads like this. Use async timers instead.
            thread::Builder::new()
                .name("window-hook-cooldown-timer".to_string())
                .spawn(move || {
                    #[allow(
                        clippy::unchecked_time_subtraction,
                        reason = "Underflow is prevented by condition above"
                    )]
                    thread::sleep(WINDOW_HOOK_COOLDOWN - elapsed);
                    if let Some(context) = WINDOW_HOOK_MANAGER.lock().unwrap().as_mut()
                        && context.last_time_notified == original_last_time_notified
                    {
                        context.tick();
                    }
                })
                .unwrap();
        }
    }
}

static WINDOW_HOOK_MANAGER: Mutex<Option<WindowHookManager>> = Mutex::new(None);

unsafe extern "system" fn win_event_hook_callback(
    _hwineventhook: windows::Win32::UI::Accessibility::HWINEVENTHOOK,
    _event: u32,
    _hwnd: windows::Win32::Foundation::HWND,
    _idobject: i32,
    _idchild: i32,
    _ideventthread: u32,
    _dwmseventtime: u32,
) {
    // TODO: try async here with a block on
    if let Some(manager) = WINDOW_HOOK_MANAGER.lock().unwrap().as_mut() {
        manager.tick();
    }
}

pub fn launch(tx: Sender<GlobalMessage>) {
    {
        let mut window_hook_context = WINDOW_HOOK_MANAGER.lock().unwrap();
        debug_assert!(window_hook_context.is_none(), "Hook already launched");
        *window_hook_context = Some(WindowHookManager::new(tx));
    }
    if let Err(e) = thread::Builder::new()
        .name("win-event-hook-loop".to_string())
        .spawn(move || unsafe {
            let hook = SetWinEventHook(
                EVENT_OBJECT_CREATE,
                EVENT_OBJECT_LOCATIONCHANGE,
                None,
                Some(win_event_hook_callback),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            );
            if !GetMessageW(null_mut(), None, 0, 0).as_bool() {
                let _ = UnhookWinEvent(hook);
                WINDOW_HOOK_MANAGER.lock().unwrap().take();
            }
        })
    {
        log::error!("Failed to launch window hook thread: {e:?}");
    }
}
