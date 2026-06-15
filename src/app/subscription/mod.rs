//! iced subscriptions: long-lived streams that feed external events (global
//! keyboard input, OS window changes) into the app as [`crate::app::Message`]s.

pub mod global;

/// Capacity of the channels bridging the OS hook threads to the iced stream.
pub const STREAM_CHANNEL_BUFFER_SIZE: usize = 100;
