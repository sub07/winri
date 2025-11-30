use std::sync::mpsc::Receiver;

use keyboard_types::Modifiers;
use rdev::{EventType, Key};

use crate::system;

#[derive(Debug)]
pub struct Event(pub Modifiers, pub Key);

const fn is_meta_key(key: Key) -> bool {
    matches!(key, Key::MetaLeft | Key::MetaRight | Key::Unknown(92))
}

pub fn launch_hook() -> Receiver<Event> {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("global-key-hook".into())
        .spawn(move || {
            let mut modifiers = Modifiers::default();
            rdev::_grab(move |event| {
                if matches!(
                    event.event_type,
                    EventType::MouseMove { .. }
                        | EventType::Wheel { .. }
                        | EventType::ButtonPress(_)
                        | EventType::ButtonRelease(_)
                ) {
                    return Some(event);
                }

                match event.event_type {
                    rdev::EventType::KeyPress(key) => {
                        match key {
                            Key::ShiftLeft | Key::ShiftRight => {
                                modifiers.set(Modifiers::SHIFT, true);
                                return Some(event);
                            }
                            Key::ControlLeft | Key::ControlRight => {
                                modifiers.set(Modifiers::CONTROL, true);
                                return Some(event);
                            }
                            Key::Alt => {
                                modifiers.set(Modifiers::ALT, true);
                                return Some(event);
                            }
                            // Got an unknown key code for the right Meta key for some reason
                            Key::MetaLeft | Key::MetaRight | Key::Unknown(92) => {
                                modifiers.set(Modifiers::META, true);
                                // Win key presses are swallowed to avoid opening the Start Menu, and triggering system shortcuts
                                // Winri is supposed to be a sort of "command center" for the system, so the native system shortcuts should not be needed
                                // I know this might be controversial, but it's the intended behavior for now, and I'm open to feedback on this matter
                                return None;
                            }
                            _ => {
                                sender.send(Event(modifiers, key)).unwrap();
                                return (!modifiers.contains(Modifiers::META)).then_some(event);
                            }
                        }
                    }
                    rdev::EventType::KeyRelease(key) => {
                        match key {
                            Key::ShiftLeft | Key::ShiftRight => {
                                modifiers.set(Modifiers::SHIFT, false);
                            }
                            Key::ControlLeft | Key::ControlRight => {
                                modifiers.set(Modifiers::CONTROL, false);
                            }
                            Key::Alt => {
                                modifiers.set(Modifiers::ALT, false);
                            }
                            // Got an unknown key code for the right Meta key for some reason
                            Key::MetaLeft | Key::MetaRight | Key::Unknown(92) => {
                                modifiers.set(Modifiers::META, false);
                                return None;
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
                Some(event)
            })
            .unwrap();
        })
        .unwrap();
    receiver
}
