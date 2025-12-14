use crate::entry::FileEntry;
use crate::state::{SearchOptions, SearchResult};
use std::path::PathBuf;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::thread::{self, JoinHandle};

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
    /// Graceful shutdown signal - worker thread will exit after receiving this
    Shutdown,
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

/// Worker thread handle for graceful shutdown
pub struct WorkerHandle {
    pub command_tx: SyncSender<IoCommand>,
    pub result_rx: Receiver<IoResult>,
    thread_handle: Option<JoinHandle<()>>,
}

impl WorkerHandle {
    /// Request graceful shutdown and wait for worker to finish
    pub fn shutdown(mut self) {
        // Send shutdown signal (ignore error if channel is closed)
        let _ = self.command_tx.send(IoCommand::Shutdown);

        // Wait for thread to finish
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

pub fn spawn_worker(ctx: eframe::egui::Context) -> WorkerHandle {
    // Use bounded channels to prevent memory exhaustion from rapid commands
    let (cmd_tx, cmd_rx) = sync_channel(COMMAND_QUEUE_CAPACITY);
    // Results channel can be larger since results are consumed quickly by UI
    let (res_tx, res_rx) = sync_channel(64);

    let ctx_clone = ctx.clone();
    let handle = thread::spawn(move || {
        while let Ok(cmd) = cmd_rx.recv() {
            match cmd {
                IoCommand::Shutdown => {
                    // Graceful shutdown - exit the loop
                    break;
                }
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

    WorkerHandle {
        command_tx: cmd_tx,
        result_rx: res_rx,
        thread_handle: Some(handle),
    }
}
