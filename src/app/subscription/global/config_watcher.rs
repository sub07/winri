use std::{path::PathBuf, sync::mpsc::channel, thread};

use iced::futures::channel::mpsc::Sender;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::app::subscription::global::GlobalMessage;

pub fn launch(mut tx: Sender<GlobalMessage>, config_source: PathBuf) {
    let _ = thread::Builder::new()
        .name("config-watcher-hook".into())
        .spawn(move || {
            let (watcher_tx, watcher_rx) = channel();
            let mut watcher: RecommendedWatcher =
                Watcher::new(watcher_tx, notify::Config::default()).unwrap();
            watcher
                .watch(&config_source, RecursiveMode::NonRecursive)
                .unwrap();

            loop {
                match watcher_rx.recv() {
                    Ok(Ok(event)) => {
                        if let EventKind::Modify(_) = event.kind {
                            tx.try_send(GlobalMessage::ConfigChanged(config_source.clone()))
                                .ok();
                        }
                    }
                    Ok(Err(e)) => log::error!("Error: {e:?}"),
                    Err(e) => log::error!("Error: {e:?}"),
                }
            }
        });
}
