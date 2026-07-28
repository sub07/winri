//! Global keyboard hook. Runs `rdev`'s grabbing hook on a dedicated thread,
//! tracks modifier state, forwards keystrokes as [`GlobalMessage::Key`], and
//! swallows the Win key so winri can use it as its modifier without the Start
//! Menu or system shortcuts firing.

use std::{thread, time::Duration};

use iced::futures::channel::mpsc::Sender;
use joy_error::ResultUtilityExt;
use keyboard_types::Modifiers;
use rdev::{EventType, Key, simulate};

use crate::{app::subscription::global::GlobalMessage, system};

pub struct State {
    pub modifiers: Modifiers,
    /// Whether the Win+L key has been pressed lately and not handled yet. Read more below.
    pub win_l_pressed: bool,
}

impl State {
    /// Injest an event, and act depending on its internal state.
    ///
    /// Returns `Some(event)` if the event should be forwarded, `None` if it should be swallowed.
    fn ingest(&mut self, event: rdev::Event, mut tx: Sender<GlobalMessage>) -> Option<rdev::Event> {
        match event.event_type {
            rdev::EventType::KeyPress(key) => {
                match key {
                    Key::ShiftLeft | Key::ShiftRight => {
                        self.modifiers.set(Modifiers::SHIFT, true);
                        return Some(event);
                    }
                    Key::ControlLeft | Key::ControlRight => {
                        self.modifiers.set(Modifiers::CONTROL, true);
                        return Some(event);
                    }
                    Key::Alt => {
                        self.modifiers.set(Modifiers::ALT, true);
                        return Some(event);
                    }
                    // Got an unknown key code for the right Meta key for some reason
                    Key::MetaLeft | Key::MetaRight | Key::Unknown(92) => {
                        self.modifiers.set(Modifiers::META, true);
                        // Win key presses are swallowed to avoid opening the Start Menu, and triggering system shortcuts
                        // Winri is supposed to be a sort of "command center" for the system, so the native system shortcuts should not be needed
                        // This might be controversial, but it's the intended behavior for now, and I'm open to feedback on this matter
                        return None;
                    }
                    _ => {
                        tx.try_send(GlobalMessage::Key(self.modifiers, key))
                            .unwrap();
                        if (self.modifiers, key) == (Modifiers::META, Key::KeyL) {
                            // Win + L is a system shortcut to lock the screen
                            // We cannot override or block it unless the DisableLockWorkstation is enabled in the registry (This is what `system::is_lock_enabled()` checks for)
                            match system::is_lock_enabled() {
                                Ok(lock_enabled) => {
                                    // Lock screen causes an unwanted behavior when triggered: The win down event is registered but not the win up event
                                    // So when on the lock screen the win key is considered down despite the user not pressing it
                                    // FIX: Simulate a win up event after the lock screen appeared
                                    if lock_enabled {
                                        log::debug!(
                                            "Lock screen is enabled, simulating win up event after lock screen appears"
                                        );
                                        thread::sleep(Duration::from_millis(300)); // Waiting for the lock screen to appear
                                        // Those simulations won't be picked up by this hook, because lockscreens prevent it.
                                        simulate(&EventType::KeyRelease(Key::MetaLeft)).discard();
                                        simulate(&EventType::KeyRelease(Key::MetaRight)).discard();
                                    } else {
                                        // Even when the lock screen is disabled, win+L press event is still considered pressed and can't be swallowed.
                                        // So we must keep track of that because the win+L release WILL be swallowed by our hook.
                                        // Without this, the win key would be considered pressed for ever.
                                        self.win_l_pressed = true;
                                    }
                                }
                                Err(err) => {
                                    log::error!(
                                        "Could not retrieve lock status, this can cause win key hold. Restart your system in that case: {err:?}"
                                    );
                                }
                            }
                        }
                        return (!self.modifiers.contains(Modifiers::META)).then_some(event);
                    }
                }
            }
            rdev::EventType::KeyRelease(key) => {
                match key {
                    Key::ShiftLeft | Key::ShiftRight => {
                        self.modifiers.set(Modifiers::SHIFT, false);
                    }
                    Key::ControlLeft | Key::ControlRight => {
                        self.modifiers.set(Modifiers::CONTROL, false);
                    }
                    Key::Alt => {
                        self.modifiers.set(Modifiers::ALT, false);
                    }
                    // Got an unknown key code for the right Meta key for some reason
                    Key::MetaLeft | Key::MetaRight | Key::Unknown(92) => {
                        self.modifiers.set(Modifiers::META, false);
                        if self.win_l_pressed {
                            self.win_l_pressed = false;
                            log::debug!(
                                "forwarding a win release event to counter the previous win+L"
                            );
                            return Some(event);
                        }
                        return None;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Some(event)
    }
}

fn grab_event_processing(
    event: rdev::Event,
    input_state: &mut State,
    tx: Sender<GlobalMessage>,
) -> Option<rdev::Event> {
    if matches!(
        event.event_type,
        EventType::MouseMove { .. }
            | EventType::Wheel { .. }
            | EventType::ButtonPress(_)
            | EventType::ButtonRelease(_)
    ) {
        return Some(event);
    }

    log::debug!("{:?}", event.event_type);

    input_state.ingest(event, tx)
}

pub fn launch(tx: Sender<GlobalMessage>) {
    let _ = thread::Builder::new()
        .name("global-key-hook".into())
        .spawn(move || {
            let mut input_state = State {
                modifiers: system::current_modifiers(),
                win_l_pressed: false,
            };
            rdev::_grab(move |event| grab_event_processing(event, &mut input_state, tx.clone()))
                .unwrap();
        });
}
