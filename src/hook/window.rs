use std::{
    ptr::null_mut,
    sync::{
        Mutex,
        mpsc::{Receiver, Sender},
    },
    thread,
    time::{Duration, Instant},
};

use windows::Win32::{
    Foundation::HWND,
    UI::{
        Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent},
        WindowsAndMessaging::{
            EVENT_OBJECT_CREATE, EVENT_OBJECT_FOCUS, GetMessageA, WINEVENT_OUTOFCONTEXT,
            WINEVENT_SKIPOWNPROCESS,
        },
    },
};

#[derive(Debug)]
pub enum HookError {
    AlreadyLaunched,
}

const WINDOW_HOOK_COOLDOWN: Duration = Duration::from_millis(200);

struct WindowHookContext {
    notifier: Sender<()>,
    last_time_notified: Instant,
}

impl WindowHookContext {
    fn new(notifier: Sender<()>) -> Self {
        Self {
            notifier,
            last_time_notified: Instant::now(),
        }
    }

    fn tick(&mut self) {
        let elapsed = self.last_time_notified.elapsed();
        if elapsed > WINDOW_HOOK_COOLDOWN {
            self.notifier.send(()).unwrap();
            self.last_time_notified = Instant::now();
        } else {
            let original_last_time_notified = self.last_time_notified;
            thread::spawn(move || {
                thread::sleep(WINDOW_HOOK_COOLDOWN - elapsed);
                if let Some(context) = WINDOW_HOOK_CHANNEL.lock().unwrap().as_mut()
                    && context.last_time_notified == original_last_time_notified
                {
                    context.tick();
                }
            });
        }
    }
}

static WINDOW_HOOK_CHANNEL: Mutex<Option<WindowHookContext>> = Mutex::new(None);

unsafe extern "system" fn hook_callback(
    _hwineventhook: HWINEVENTHOOK,
    _event: u32,
    _hwnd: HWND,
    _idobject: i32,
    _idchild: i32,
    _ideventthread: u32,
    _dwmseventtime: u32,
) {
    if let Some(context) = WINDOW_HOOK_CHANNEL.lock().unwrap().as_mut() {
        context.tick();
    }
}

pub fn launch_window_hook() -> Result<Receiver<()>, HookError> {
    if WINDOW_HOOK_CHANNEL.lock().unwrap().is_some() {
        Err(HookError::AlreadyLaunched)
    } else {
        let (sender, receiver) = std::sync::mpsc::channel();
        *WINDOW_HOOK_CHANNEL.lock().unwrap() = Some(WindowHookContext::new(sender));
        thread::spawn(|| unsafe {
            let hook = SetWinEventHook(
                EVENT_OBJECT_CREATE,
                EVENT_OBJECT_FOCUS,
                None,
                Some(hook_callback),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            );
            if !GetMessageA(null_mut(), None, 0, 0).as_bool() {
                let _ = UnhookWinEvent(hook);
                WINDOW_HOOK_CHANNEL.lock().unwrap().take();
            }
        });
        Ok(receiver)
    }
}
