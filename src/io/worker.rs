use crate::entry::FileEntry;
use crate::state::{SearchOptions, SearchResult};
use std::path::PathBuf;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::thread;

use super::directory::read_directory;
use super::search::perform_search;

/// Maximum number of pending commands in the worker queue.
/// This prevents memory exhaustion from rapid command submissions.
const COMMAND_QUEUE_CAPACITY: usize = 16;

pub enum IoCommand {
    LoadDirectory(PathBuf, bool),
    LoadParent(PathBuf, bool),
    SearchContent {
        query: String,
        root_path: PathBuf,
        options: SearchOptions,
    },
}

pub enum IoResult {
    DirectoryLoaded {
        path: PathBuf,
        entries: Vec<FileEntry>,
    },
    ParentLoaded(Vec<FileEntry>),
    SearchCompleted(Vec<SearchResult>),
    SearchProgress {
        files_searched: usize,
        files_skipped: usize,
        errors: usize,
    },
    Error(String),
}

pub fn spawn_worker(
    ctx: eframe::egui::Context,
) -> (SyncSender<IoCommand>, Receiver<IoResult>) {
    // Use bounded channels to prevent memory exhaustion from rapid commands
    let (cmd_tx, cmd_rx) = sync_channel(COMMAND_QUEUE_CAPACITY);
    // Results channel can be larger since results are consumed quickly by UI
    let (res_tx, res_rx) = sync_channel(64);

    let ctx_clone = ctx.clone();
    thread::spawn(move || {
        while let Ok(cmd) = cmd_rx.recv() {
            match cmd {
                IoCommand::LoadDirectory(path, hidden) => match read_directory(&path, hidden) {
                    Ok(entries) => {
                        let _ = res_tx.send(IoResult::DirectoryLoaded {
                            path: path.clone(),
                            entries,
                        });
                    }
                    Err(e) => {
                        let _ = res_tx.send(IoResult::Error(e.to_string()));
                    }
                },
                IoCommand::LoadParent(path, hidden) => match read_directory(&path, hidden) {
                    Ok(entries) => {
                        let _ = res_tx.send(IoResult::ParentLoaded(entries));
                    }
                    Err(_) => {
                        let _ = res_tx.send(IoResult::ParentLoaded(Vec::new()));
                    }
                },
                IoCommand::SearchContent {
                    query,
                    root_path,
                    options,
                } => match perform_search(&query, &root_path, &options, &res_tx) {
                    Ok(results) => {
                        let _ = res_tx.send(IoResult::SearchCompleted(results));
                    }
                    Err(e) => {
                        let _ = res_tx.send(IoResult::Error(format!("Search error: {}", e)));
                    }
                },
            }
            ctx_clone.request_repaint();
        }
    });

    (cmd_tx, res_rx)
}
