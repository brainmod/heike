use crate::entry::FileEntry;
use crate::state::{SearchOptions, SearchResult};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use super::directory::read_directory;
use super::search::perform_search;

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
    SearchProgress(usize),
    Error(String),
}

pub fn spawn_worker(
    ctx: eframe::egui::Context,
) -> (Sender<IoCommand>, Receiver<IoResult>) {
    let (cmd_tx, cmd_rx) = channel();
    let (res_tx, res_rx) = channel();

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
