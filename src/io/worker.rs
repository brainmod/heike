use crate::entry::{FileEntry, GitStatus};
use crate::state::{SearchOptions, SearchResult};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::thread;

use super::directory::{git_statuses, read_directory};
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
    /// Sent after DirectoryLoaded/ParentLoaded so listings show without waiting on git
    GitStatusLoaded {
        path: PathBuf,
        statuses: HashMap<String, GitStatus>,
    },
    SearchCompleted(Vec<SearchResult>),
    SearchProgress {
        files_searched: usize,
        files_skipped: usize,
        errors: usize,
    },
    Error(String),
    SearchError(String),
}

/// Channels to the worker thread. The thread exits when `command_tx` is dropped.
pub struct WorkerHandle {
    pub command_tx: SyncSender<IoCommand>,
    pub result_rx: Receiver<IoResult>,
}

pub fn spawn_worker(ctx: eframe::egui::Context) -> WorkerHandle {
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
                        ctx_clone.request_repaint();
                        let statuses = git_statuses(&path);
                        let _ = res_tx.send(IoResult::GitStatusLoaded { path, statuses });
                    }
                    Err(e) => {
                        let _ = res_tx.send(IoResult::Error(e.to_string()));
                    }
                },
                IoCommand::LoadParent(path, hidden) => match read_directory(&path, hidden) {
                    Ok(entries) => {
                        let _ = res_tx.send(IoResult::ParentLoaded(entries));
                        ctx_clone.request_repaint();
                        let statuses = git_statuses(&path);
                        let _ = res_tx.send(IoResult::GitStatusLoaded { path, statuses });
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
                        let _ = res_tx.send(IoResult::SearchError(format!("Search error: {}", e)));
                    }
                },
            }
            ctx_clone.request_repaint();
        }
    });

    WorkerHandle {
        command_tx: cmd_tx,
        result_rx: res_rx,
    }
}
