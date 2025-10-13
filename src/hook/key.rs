use std::sync::mpsc::Receiver;

use bitflags::bitflags;
use rdev::Key;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Modifiers: u8 {
        const SHIFT = 1 << 0;
        const CTRL = 1 << 1;
        const ALT = 1 << 2;
        const WIN = 1 << 3;
    }
}

pub struct Event(pub Modifiers, pub Key);

pub fn launch_hook() -> Receiver<Event> {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut modifiers = Modifiers::empty();
        rdev::_grab(move |event| {
            match event.event_type {
                rdev::EventType::KeyPress(key) => match dbg!(key) {
                    rdev::Key::ShiftLeft | rdev::Key::ShiftRight => {
                        modifiers.insert(Modifiers::SHIFT);
                    }
                    rdev::Key::ControlLeft | rdev::Key::ControlRight => {
                        modifiers.insert(Modifiers::CTRL);
                    }
                    rdev::Key::Alt => modifiers.insert(Modifiers::ALT),
                    rdev::Key::MetaLeft | rdev::Key::Unknown(92) => {
                        modifiers.insert(Modifiers::WIN);
                    }
                    key => {
                        sender.send(Event(modifiers, key)).unwrap();
                        return (!modifiers.contains(Modifiers::WIN)).then_some(event);
                    }
                },
                rdev::EventType::KeyRelease(key) => match key {
                    rdev::Key::ShiftLeft | rdev::Key::ShiftRight => {
                        modifiers.remove(Modifiers::SHIFT);
                    }
                    rdev::Key::ControlLeft | rdev::Key::ControlRight => {
                        modifiers.remove(Modifiers::CTRL);
                    }
                    rdev::Key::Alt => modifiers.remove(Modifiers::ALT),
                    rdev::Key::MetaLeft | rdev::Key::Unknown(92) => {
                        modifiers.remove(Modifiers::WIN);
                    }
                    _ => {}
                },
                _ => {}
            }
            Some(event)
        })
        .unwrap();
    });
    receiver
}
