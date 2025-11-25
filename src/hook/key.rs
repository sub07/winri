use std::sync::mpsc::Receiver;

use keyboard_types::Modifiers;
use rdev::{EventType, Key};

use crate::system;

pub struct Event(pub Modifiers, pub Key);

const fn is_meta_key(key: Key) -> bool {
    matches!(key, Key::MetaLeft | Key::MetaRight | Key::Unknown(92))
}

pub fn launch_hook() -> Receiver<Event> {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("global-key-hook".into())
        .spawn(move || {
            let mut meta_pressed = false;
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

                let mut modifiers = system::current_modifiers();
                modifiers.set(Modifiers::META, meta_pressed);
                log::debug!("{modifiers:?}");

                match event.event_type {
                    rdev::EventType::KeyPress(key) => {
                        if is_meta_key(key) {
                            meta_pressed = true;
                            return None;
                        }
                        sender.send(Event(modifiers, key)).unwrap();
                        return (!meta_pressed).then_some(event);
                    }
                    rdev::EventType::KeyRelease(key) => {
                        if is_meta_key(key) {
                            meta_pressed = false;
                            return None;
                        }
                        return (!meta_pressed).then_some(event);
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
