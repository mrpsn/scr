mod args;
pub mod util;

use crate::args::Args;
use crate::util::print::display_time;
use core::time::Duration;
use rayon::prelude::*;
use sorted_vec::ReverseSortedVec;
use std::cmp::{Ordering, Reverse};
use std::io::ErrorKind;
use std::ops::AddAssign;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;
use std::thread;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::time::Instant;
use util::print::FilePrinter;

pub enum StatusMsg<'a> {
    Status(&'a ScanResult),
    Final(ScanResult, Duration),
}

enum StatusUpdate {
    Result(ScanResult),
    File(Filesize),
}

impl From<PathBuf> for StatusUpdate {
    fn from(path: PathBuf) -> Self {
        let meta = path.metadata().unwrap();
        StatusUpdate::File(Filesize {
            path: path.to_str().unwrap().to_string(),
            size: path.metadata().unwrap().len(),
            modified: display_time(meta.modified()),
            created: display_time(meta.created()),
            used: display_time(meta.accessed()),
        })
    }
}

#[derive(PartialOrd, Eq, Clone)]
pub struct Filesize {
    path: String,
    size: u64,
    modified: String,
    created: String,
    used: String,
}

impl Ord for Filesize {
    fn cmp(&self, other: &Self) -> Ordering {
        self.size.cmp(&other.size)
    }
}

impl PartialEq for Filesize {
    fn eq(&self, other: &Self) -> bool {
        self.size == other.size
    }
}

#[derive(Default)]
pub struct ScanResult {
    errors: usize,
    permission_denied: usize,
    files: usize,
    directories: usize,
}

impl AddAssign for ScanResult {
    fn add_assign(&mut self, other: Self) {
        self.errors += other.errors;
        self.permission_denied += other.permission_denied;
        self.files += other.files;
        self.directories += other.directories;
    }
}

fn scan_dir(
    path: PathBuf,
    min_size: Arc<AtomicU64>,
    tx_file: UnboundedSender<StatusUpdate>,
) -> ScanResult {
    let mut result = ScanResult::default();

    match std::fs::read_dir(path) {
        Ok(dir_iter) => {
            // Collect entries to parallelize
            let entries: Vec<_> = dir_iter.filter_map(|r| r.ok()).collect();

            let results: Vec<ScanResult> = entries
                .par_iter()
                .map(|e| {
                    let mut thread_result = ScanResult::default();
                    match e.file_type() {
                        Ok(ft) if ft.is_dir() => {
                            thread_result += scan_dir(e.path(), min_size.clone(), tx_file.clone());
                        }
                        Ok(ft) => {
                            if ft.is_symlink() {
                                thread_result.files += 1;
                            } else {
                                match e.metadata() {
                                    Ok(m) => {
                                        let current_min = min_size.load(SeqCst);
                                        if m.len() >= current_min {
                                            tx_file.send(e.path().into()).unwrap_or_default();
                                            thread_result.files += 1;
                                        } else {
                                            thread_result.files += 1;
                                        }
                                    }
                                    Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                                        thread_result.permission_denied += 1
                                    }
                                    Err(_) => thread_result.errors += 1,
                                }
                            }
                        }
                        Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                            thread_result.permission_denied += 1
                        }
                        Err(_) => thread_result.errors += 1,
                    }
                    thread_result
                })
                .collect();

            // Should verify this doesn't double count directories?
            result.directories += 1;
            for r in results {
                result += r;
            }
        }
        Err(e) if e.kind() == ErrorKind::PermissionDenied => result.permission_denied += 1,
        Err(_) => result.errors += 1,
    }
    result
}

fn print_files(min_size: Arc<AtomicU64>, mut rx_file: UnboundedReceiver<StatusUpdate>) {
    let start_time = Instant::now();

    let n = Args::parse_args().nentries;
    let mut printer = FilePrinter::new("");

    let mut entries = ReverseSortedVec::<Filesize>::with_capacity(n);
    let mut current_status = ScanResult::default();

    while let Some(msg) = rx_file.blocking_recv() {
        match msg {
            StatusUpdate::Result(sr) => {
                current_status += sr;
                if current_status.directories % 10 == 0 {
                    printer.draw(&entries, &current_status, None);
                }
            }

            StatusUpdate::File(file) => {
                let current_min = min_size.load(SeqCst);
                if file.size > current_min {
                    entries.insert(Reverse(file));

                    // The vector is sorted in reverse (largest size first).
                    // If we exceed the limit, the last element is the smallest of the top N+1,
                    // so we remove it.
                    if entries.len() > n {
                        entries.pop();
                    }

                    if entries.len() == n {
                        if let Some(entry) = entries.last() {
                            min_size.store(entry.0.size, SeqCst);
                        }
                    }

                    printer.draw(&entries, &current_status, None);
                }
            }
        }
    }
    let end_time = Instant::now();
    let elapsed_time = end_time - start_time;
    printer.print_final(&entries, &current_status, elapsed_time);
}

#[tokio::main]
async fn main() {
    let args = Args::parse_args();

    let (tx, rx) = unbounded_channel::<StatusUpdate>();

    let floor = Arc::new(AtomicU64::new(args.minsize));
    let floor_clone = Arc::clone(&floor);

    let t1 = thread::Builder::new()
        .name("file_printer".into())
        .spawn(move || print_files(floor_clone, rx))
        .unwrap();

    let root = args.path.clone();
    let floor_clone_2 = Arc::clone(&floor);

    let handle = tokio::task::spawn_blocking(move || {
        let final_res = scan_dir(root, floor_clone_2, tx.clone());
        tx.send(StatusUpdate::Result(final_res)).unwrap();
    });

    handle.await.unwrap();

    t1.join().unwrap();
}
