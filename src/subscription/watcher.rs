use crate::message::Message;
use iced::futures::SinkExt;
use iced::stream;
use iced::Subscription;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::hash::Hash;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FileWatcherId(PathBuf);

pub fn file_watcher(path: PathBuf) -> Subscription<Message> {
    Subscription::run_with_id(
        FileWatcherId(path.clone()),
        stream::channel(100, move |mut output| async move {
            let path_clone = path.clone();
            let (tx, mut rx) = tokio::sync::mpsc::channel(10);

            let mut watcher: RecommendedWatcher =
                match notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
                    if res.is_ok() {
                        let _ = tx.blocking_send(());
                    }
                }) {
                    Ok(w) => w,
                    Err(_) => return,
                };

            if watcher
                .watch(&path, RecursiveMode::NonRecursive)
                .is_err()
            {
                return;
            }

            loop {
                if rx.recv().await.is_some() {
                    let _ = output
                        .send(Message::FileWatcherEvent(path_clone.clone()))
                        .await;
                }
            }
        }),
    )
}
