use std::{thread, time::Duration};

use iced::futures::channel::mpsc::Sender;
use joy_error::ResultUtilityExt;
use keyboard_types::Modifiers;
use rdev::simulate;

use crate::{app::subscription::global::GlobalMessage, system};

fn grab_event_processing(
    event: rdev::Event,
    modifiers: &mut Modifiers,
    mut tx: Sender<GlobalMessage>,
) -> Option<rdev::Event> {
    use rdev::{EventType, Key};

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
                    tx.try_send(GlobalMessage::Key(*modifiers, key)).unwrap();
                    if (*modifiers, key) == (Modifiers::META, Key::KeyL) {
                        // Win + L is a system shortcut to lock the screen
                        // We cannot override or block it
                        // It causes an unwanted behavior when triggering it causing the win key down to be registered but not the win up
                        // So when on the lock screen the win key is considered down despite the user not pressing it
                        // FIX: Simulate a win up event after the lock screen appeared by wait after a delay
                        thread::sleep(Duration::from_millis(300));
                        simulate(&EventType::KeyRelease(Key::MetaLeft)).discard();
                        simulate(&EventType::KeyRelease(Key::MetaRight)).discard();
                    }
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
}

pub fn launch(tx: Sender<GlobalMessage>) {
    let _ = thread::Builder::new()
        .name("global-key-hook".into())
        .spawn(move || {
            let mut modifiers = system::current_modifiers();
            rdev::_grab(move |event| grab_event_processing(event, &mut modifiers, tx.clone()))
                .unwrap();
        });
}
