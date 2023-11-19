mod args;
pub mod util;

use crate::util::print::{print_footer, display_time};
use bisection::bisect_left;
use futures::future::join_all;
use sorted_vec::ReverseSortedVec;
use std::cmp::{Ordering, Reverse};
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;
use std::{thread};
use std::iter::Sum;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::time::Instant;
use util::print::FilePrinter;
use crate::args::Args;


struct Dir {
    path: PathBuf,
    tx_dir: UnboundedSender<Dir>,
    tx_file: UnboundedSender<Filesize>,
}

#[derive(PartialOrd, Eq, Clone)]
pub struct Filesize {
    path: String,
    size: u64,
    modified: String,
    created: String,
    used: String,
}


impl From<PathBuf> for Filesize {
    fn from(path: PathBuf) -> Self {
        let meta = path.metadata().unwrap();
        Self {
            path: path.to_str().unwrap().to_string(),
            size: path.metadata().unwrap().len(),
            modified: display_time(meta.modified()),
            created: display_time(meta.created()),
            used: display_time(meta.accessed()),
        }
    }
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

impl Sum for ScanResult {
    fn sum<I>(iter: I) -> Self
        where
            I: Iterator<Item = Self>,
    {
        iter.fold(Self::default(), |acc, val| Self {
            errors: acc.errors + val.errors,
            files: acc.files + val.files,
            directories: acc.directories + val.directories,
        })
    }
}


async fn scan_dir(
    path: PathBuf,
    min_size: u64,
    tx_file: UnboundedSender<Filesize>,
    tx_dir: UnboundedSender<Dir>,
) -> ScanResult {
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
    ScanResult{errors, files, directories: 1}
}


fn print_files(min_size: Arc<AtomicU64>, mut rx_file: UnboundedReceiver<Filesize>) {
    let n = Args::parse_args().nentries;
    let mut printer = FilePrinter::new(
        &format!("Scanning for largest {n} files.."),
    );

    let mut entries = ReverseSortedVec::<Filesize>::with_capacity(n);

    while let Some(file) = rx_file.blocking_recv() {
        let current_min = match entries.len() >= n {
            true => entries.last().expect("can't unwrap last entry").0.size,
            false => min_size.load(SeqCst),
        };

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

                let n_lines = n.min(entries.len());
                for (i, entry) in entries[idx..n_lines].iter().enumerate() {
                    printer.print_line(entry.0.clone(), (idx + i) as u16);
                }
            }
        }
    }
    printer.print_final(n);
}

#[tokio::main]
async fn main() {
    let args = Args::parse_args();

    let file_ch = unbounded_channel::<Filesize>();
    let mut dir_ch = unbounded_channel::<Dir>();

    let floor = Arc::new(AtomicU64::new(args.minsize));
    let floor_clone = Arc::clone(&floor);
    let start_time = Instant::now();

    let t = thread::Builder::new()
        .name("file_printer".into())
        .spawn(move ||
            print_files(
                floor_clone,
                file_ch.1,
            )
        ).unwrap();

    let init = move ||
        dir_ch.0.send(
            Dir{path: args.path, tx_dir: dir_ch.0.clone(), tx_file: file_ch.0}
        ).unwrap();
    init();

    let mut scans = vec![];
    while let Some(dir) = dir_ch.1.recv().await {
        scans.push(tokio::spawn(scan_dir(
            dir.path,
            floor.load(SeqCst),
            dir.tx_file,
            dir.tx_dir,
        )));
    }

    let total: ScanResult  = join_all(scans)
        .await
        .into_iter()
        .map(|r| r.unwrap_or_default())
        .sum();

    t.join().expect("failed to complete printing");

    let end_time = Instant::now();
    let elapsed_time = end_time - start_time;
    print_footer(elapsed_time, total);
}
