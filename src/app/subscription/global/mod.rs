mod input;
mod window;

use iced::{
    futures::{SinkExt, Stream, StreamExt, channel::mpsc::channel},
    stream,
};
use keyboard_types::Modifiers;

use crate::app::{Message, subscription::STREAM_CHANNEL_BUFFER_SIZE};

#[derive(Debug, Clone)]
pub enum GlobalMessage {
    Key(Modifiers, rdev::Key),
    Window,
}

pub fn subscription() -> impl Stream<Item = Message> {
    stream::channel(STREAM_CHANNEL_BUFFER_SIZE, async |mut output| {
        let (intermediate_message_tx, mut intermediate_message_rx) = channel(100);

        let global_input_tx = intermediate_message_tx.clone();
        let window_event_tx = intermediate_message_tx;

        input::launch(global_input_tx);
        window::launch(window_event_tx);

        while let Some(event) = intermediate_message_rx.next().await {
            output.send(Message::Global(event)).await.unwrap();
        }
    })
}
