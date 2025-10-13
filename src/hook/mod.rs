use std::sync::mpsc::Receiver;

pub mod key;
pub mod window;

pub enum Event {
    Key(key::Event),
    Window,
}

pub fn launch_hooks() -> anyhow::Result<Receiver<Event>> {
    let window_event_receiver = window::launch_hook()?;
    let key_event_receiver = key::launch_hook();

    let (sender, receiver) = std::sync::mpsc::channel();

    let window_event_sender = sender.clone();
    let key_event_sender = sender;

    std::thread::spawn(move || {
        for () in window_event_receiver {
            window_event_sender.send(Event::Window).unwrap();
        }
    });

    std::thread::spawn(move || {
        for key_event in key_event_receiver {
            key_event_sender.send(Event::Key(key_event)).unwrap();
        }
    });

    Ok(receiver)
}
