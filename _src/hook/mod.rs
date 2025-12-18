use std::sync::mpsc::Sender;

use crate::Event;

pub mod key;
// pub mod thumbnail;
pub mod window;

pub fn launch_hooks(event_tx: Sender<Event>) -> anyhow::Result<()> {
    let window_event_receiver = window::launch_hook()?;
    let key_event_receiver = key::launch_hook();

    let window_event_tx = event_tx.clone();
    let key_event_tx = event_tx;

    std::thread::Builder::new()
        .name("window-event-forwarder".into())
        .spawn(move || {
            for () in window_event_receiver {
                window_event_tx.send(Event::Window).unwrap();
            }
        })
        .unwrap();

    std::thread::Builder::new()
        .name("key-event-forwarder".into())
        .spawn(move || {
            for key_event in key_event_receiver {
                key_event_tx.send(Event::Key(key_event)).unwrap();
            }
        })
        .unwrap();

    Ok(())
}
