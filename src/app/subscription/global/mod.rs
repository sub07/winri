mod config_watcher;
mod input;
mod window;

use std::path::PathBuf;

use iced::{
    futures::{SinkExt, Stream, StreamExt, channel::mpsc::channel},
    stream,
};
use keyboard_types::Modifiers;

use crate::app::{Message, subscription::STREAM_CHANNEL_BUFFER_SIZE};

/// A raw, mode-agnostic event from the OS-level hooks.
#[derive(Debug, Clone)]
pub enum GlobalMessage {
    Key(Modifiers, rdev::Key),
    /// Some window was created or moved; the tiler should re-sync.
    Window,
    ConfigChanged(PathBuf),
}

/// The global subscription stream: launches the keyboard and window hooks and
/// merges their events into a single [`Message`] stream for the app.
#[allow(clippy::ref_option)]
pub fn subscription(config_source: &Option<PathBuf>) -> impl Stream<Item = Message> + use<> {
    let config_source = config_source.clone();
    stream::channel(STREAM_CHANNEL_BUFFER_SIZE, async |mut output| {
        let (intermediate_message_tx, mut intermediate_message_rx) = channel(100);

        let global_input_tx = intermediate_message_tx.clone();
        let window_event_tx = intermediate_message_tx.clone();
        let config_watcher_event_tx = intermediate_message_tx.clone();

        input::launch(global_input_tx);
        window::launch(window_event_tx);
        if let Some(config_source) = config_source {
            config_watcher::launch(config_watcher_event_tx, config_source);
        }

        while let Some(event) = intermediate_message_rx.next().await {
            output.send(Message::Global(event)).await.unwrap();
        }
    })
}
