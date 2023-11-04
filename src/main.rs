pub mod util;
use std::cmp::{Ordering, Reverse};
use std::os::windows::prelude::*;
use std::path::{PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64};
use std::sync::atomic::Ordering::SeqCst;
use std::thread;
use std::time::{SystemTime};
use bisection::bisect_left;
use chrono::{DateTime, Utc};
use num_format::{Locale, ToFormattedString};
use clap::Parser;
use futures::future::join_all;
use sorted_vec::ReverseSortedVec;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::time::Instant;
use util::print::{FilePrinter};
use crate::util::print::print_footer;


/// A fast directory tree scanner, listing the top n files in the tree
/// by size. Intended use, is to help quickly identify which files are
/// consuming space on your drive.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// A valid directory path to start scanning from. Defaults to '.'
    #[arg(index=1, value_name = "PATH")]
    path: PathBuf,

    /// Find files >= to size (in bytes).
    #[arg(short, long, value_name = "MINSIZE", default_value_t=0)]
    minsize: u64,

    /// number of entries to display
    #[arg(short, long, value_name = "N_ENTRIES", default_value_t=10)]
    nentries: usize,
}


struct Dir {
    path: PathBuf,
    tx_dir: UnboundedSender<Dir>,
    tx_file: UnboundedSender<Filesize>
}


#[derive(PartialOrd, Eq, Clone)]
struct Filesize {
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


fn display_time(sys_time: SystemTime) -> String {
    let datetime: DateTime<Utc> = sys_time.into();
    datetime.format("%Y-%m-%d").to_string()
}


async fn scan_dir(path: PathBuf, min_size: u64, tx_file: UnboundedSender<Filesize>, tx_dir: UnboundedSender<Dir>) -> (usize, usize) {
    let mut file_count: usize = 0;
    let mut errors: usize = 0;
    if let Ok(dir_iter) = std::fs::read_dir(path) {
        for entry in dir_iter {
            if let Ok(entry) = entry {
                if let Ok(typ) = entry.file_type() {
                    if typ.is_file() {
                        file_count += 1;
                        if let Ok(metadata) = entry.metadata() {
                            let size = metadata.file_size();
                            if size >= min_size {
                                let modified = metadata.modified().map_or("".into(), |d|display_time(d));
                                let created = metadata.created().map_or("".into(), |d|display_time(d));
                                let used = metadata.accessed().map_or("".into(), |d|display_time(d));
                                if let Some(file_path) = entry.path().to_str() {
                                    let path_str = file_path.replace("\\", "/").replace("\"", "");
                                    tx_file.send(Filesize { path: path_str, size, modified, created, used }).expect("failed to send file size on async channel");
                                } else {
                                    errors += 1;
                                }
                            }
                        }
                    } else {
                        tx_dir.send(
                            Dir {
                                path: entry.path(),
                                tx_dir: tx_dir.clone(),
                                tx_file: tx_file.clone()
                            }).expect("failed to send directory entry on async channel");
                    }
                }
            }
        }
    }
    return (file_count, errors)
}



fn print_files(n: usize, min_size:  Arc<AtomicU64>, mut rx_file: UnboundedReceiver<Filesize>) {

    let mut printer = FilePrinter::new(&format!("Scanning for largest {n} files.."));
    let mut entries = ReverseSortedVec::<Filesize>::new();

    while let Some(file) = rx_file.blocking_recv() {

        let current_min = match entries.len() >= n {
            true => entries.last().expect("can't unwrap last entry").0.size,
            false => min_size.load(SeqCst)
        };

        if file.size > current_min {
            let r = Reverse(file);
            let idx = bisect_left(&entries, &r);
            if idx <= n {
                entries.insert(r);
                for (i, entry) in entries[0..idx].iter().enumerate() {
                    let formatted_size = entry.0.size.to_formatted_string(&Locale::en);
                    let line = format!("{formatted_size:>15} {:>10} {:>10} {:>10} {}", entry.0.created, entry.0.modified, entry.0.used, entry.0.path);
                    printer.print_line(line, i as u16);
                }

                if entries.len() == n {
                    if let Some(entry) = entries.last() {
                        min_size.store(entry.0.size, SeqCst);
                    }
                }
            }
        }
    };
    printer.close();

}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let start_dir = PathBuf::from(args.path);

    match std::fs::read_dir(&start_dir) {
        Ok(_) => { },
        Err(e) => {
            println!("{e:?}");
            return
        }
    }

    let (tx_file, rx_file) = unbounded_channel::<Filesize>();
    let (tx_dir, mut rx_dir) = unbounded_channel::<Dir>();

    let floor = Arc::new(AtomicU64::new(args.minsize));
    let floor_clone = Arc::clone(&floor);

    let start_time = Instant::now();

    let t = thread::spawn( move ||
        print_files(
            args.nentries,
            floor_clone,
            rx_file
        )
    );

    let mut scans = vec![];

    scans.push(tokio::spawn(
        scan_dir(
            start_dir,
            args.minsize,
            tx_file,
            tx_dir
        )
    ));

    let mut dirs: usize = 1;
    while let Some(dir) = rx_dir.recv().await {
        scans.push(tokio::spawn(
            scan_dir(
                dir.path,
                floor.load(SeqCst),
                dir.tx_file,
                dir.tx_dir
            )
        ));
        dirs += 1;
    };

    let file_counts = join_all(scans).await;
    t.join().expect("failed to complete printer thread");
    let file_count: usize = file_counts.iter().map(|i| i.as_ref().unwrap().0).sum();
    let error_count: usize = file_counts.into_iter().map(|i| i.unwrap().1).sum();
    print_footer(start_time, file_count, error_count, dirs);
}



