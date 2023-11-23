mod args;
pub mod util;

use crate::util::print::{display_time};
use bisection::bisect_left;
use futures::future::join_all;
use sorted_vec::ReverseSortedVec;
use std::cmp::{Ordering, Reverse};
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;
use std::{thread};
use core::time::Duration;
use std::ops::AddAssign;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::time::Instant;
use util::print::FilePrinter;
use crate::args::Args;


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
        StatusUpdate::File(
            Filesize {
                path: path.to_str().unwrap().to_string(),
                size: path.metadata().unwrap().len(),
                modified: display_time(meta.modified()),
                created: display_time(meta.created()),
                used: display_time(meta.accessed()),
            }
        )
    }
}

struct Dir {
    path: PathBuf,
    tx_dir: UnboundedSender<Dir>,
    tx_file: UnboundedSender<StatusUpdate>,
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


#[derive(Clone, Default)]
pub struct ScanResult {
    errors: usize,
    files: usize,
    directories: usize,
}

impl AddAssign for ScanResult {
    fn add_assign(&mut self, other: Self) {
        self.errors += other.errors;
        self.files += other.files;
        self.directories += other.directories;
    }
}


async fn scan_dir(
    path: PathBuf,
    min_size: u64,
    tx_file: UnboundedSender<StatusUpdate>,
    tx_dir: UnboundedSender<Dir>,
) {
    let mut errors: usize = 0;
    let mut files: usize = 0;

    if let Ok(dir_iter) = std::fs::read_dir(path) {
        for r in dir_iter {
            match r {

                Ok(e) if e.file_type().is_ok_and(|f| f.is_dir()) => tx_dir.send(
                    Dir{path: e.path(), tx_dir: tx_dir.clone(), tx_file: tx_file.clone()})
                                .expect("failed to send dir on channel"),

                Ok(e) if e.metadata().is_ok_and(|m| m.len() >= min_size) =>
                    tx_file.send(e.path().into()).map_or_else(
                        |_| errors +=1, |_| files +=1),

                Ok(e) if e.metadata().is_err() => errors += 1,

                Ok(_) => files +=1,  // file loaded ok, but < the minimum size

                Err(_) => errors +=1,
            }
        };
    } else {
        errors += 1;
    };
    tx_file.send(StatusUpdate::Result(ScanResult { errors, files, directories: 1 })).unwrap();
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
                printer.print_status(StatusMsg::Status(&current_status));
            },

            StatusUpdate::File(file) => {
                let current_min = min_size.load(SeqCst);
                if file.size > current_min {
                    let r = Reverse(file);
                    let idx = bisect_left(&entries, &r);
                    if idx <= n {
                        entries.insert(r);
                        while entries.len() > n {
                            entries.pop();
                        }

                        if entries.len() == n {
                            if let Some(entry) = entries.last() {
                                min_size.store(entry.0.size, SeqCst);
                            }
                        }

                        let n_lines = n.min(entries.len()).min(printer.page_size);
                        if idx <= printer.page_size {
                            for (i, entry) in entries[idx..n_lines].iter().enumerate() {
                                printer.print_line(&entry.0, idx + i);
                            }
                        }
                    }
                }
            }
        }
    }
    let end_time = Instant::now();
    let elapsed_time = end_time - start_time;
    printer.print_final(entries, StatusMsg::Final(current_status, elapsed_time));
}


#[tokio::main]
async fn main() {
    let args = Args::parse_args();

    let file_ch = unbounded_channel::<StatusUpdate>();

    let floor = Arc::new(AtomicU64::new(args.minsize));
    let floor_clone = Arc::clone(&floor);

    let t1 = thread::Builder::new()
        .name("file_printer".into())
        .spawn(move ||
            print_files(
                floor_clone,
                file_ch.1
            )
        ).unwrap();

    let init = move |path| {
        let dir_ch = unbounded_channel::<Dir>();
        dir_ch.0.send(
            Dir{path, tx_dir: dir_ch.0.clone(), tx_file: file_ch.0}
        ).unwrap();
        dir_ch.1
    };
    let mut dir_ch = init(args.path);

    let mut scans = vec![];
    while let Some(dir) = dir_ch.recv().await {
        scans.push(tokio::spawn(scan_dir(
            dir.path,
            floor.load(SeqCst),
            dir.tx_file,
            dir.tx_dir,
        )));
    }

    join_all(scans).await;

    t1.join().unwrap();

}
