mod args;
pub mod util;

use crate::util::print::{print_footer, display_time};
use args::Args;
use bisection::bisect_left;
use clap::Parser;
use futures::future::join_all;
use num_format::{Locale, ToFormattedString};
use sorted_vec::ReverseSortedVec;
use std::cmp::{Ordering, Reverse};
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;
use std::{thread};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::time::Instant;
use util::print::FilePrinter;

struct Dir {
    path: PathBuf,
    tx_dir: UnboundedSender<Dir>,
    tx_file: UnboundedSender<Filesize>,
}

#[derive(PartialOrd, Eq, Clone)]
struct Filesize {
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



async fn scan_dir(
    path: PathBuf,
    min_size: u64,
    tx_file: UnboundedSender<Filesize>,
    tx_dir: UnboundedSender<Dir>,
) -> (usize, usize) {
    let mut file_count: usize = 0;
    let mut error_count: usize = 0;

    let send_dir = |p: PathBuf|  tx_dir.send(
        Dir{
            path: p,
            tx_dir: tx_dir.clone(),
            tx_file: tx_file.clone()
        }
    ).expect("failed to send dir");

    let send_file = |p: PathBuf|  tx_file.send(Filesize::from(p));

    if let Ok(dir_iter) = std::fs::read_dir(path) {
        dir_iter.into_iter()
            .for_each(|r| match r {
                Ok(e) if e.file_type().is_ok_and(|f| f.is_dir()) => send_dir(e.path()),
                Ok(e) if e.metadata().is_ok_and(|m| m.len() >= min_size) => send_file(e.path()).map_or_else(
                    |_| error_count += 1,
                    |_| file_count += 1
                ),
                Ok(e) if e.file_type().is_ok_and(|f| f.is_file()) => file_count +=1,
                _ => error_count += 1,
            });
        }
    return (file_count, error_count);
}

fn print_files(n: usize, min_size: Arc<AtomicU64>, mut rx_file: UnboundedReceiver<Filesize>) {
    let mut printer = FilePrinter::new(&format!("Scanning for largest {n} files.."));
    let mut entries = ReverseSortedVec::<Filesize>::new();

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
                let n_lines = n.min(entries.len());
                for (i, entry) in entries[0..n_lines].iter().enumerate() {
                    let formatted_size = entry.0.size.to_formatted_string(&Locale::en);
                    let line = format!(
                        "{formatted_size:>15}  {:>10}  {:>10}  {:>10}  {}",
                        entry.0.created, entry.0.modified, entry.0.used, entry.0.path
                    );
                    printer.print_line(line, i as u16);
                }

                if entries.len() == n {
                    if let Some(entry) = entries.last() {
                        min_size.store(entry.0.size, SeqCst);
                    }
                }
            }
        }
    }
    printer.close();
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    match std::fs::read_dir(&args.path) {
        Ok(_) => {}
        Err(e) => {
            println!("{e:?}");
            return;
        }
    }

    let file_ch = unbounded_channel::<Filesize>();
    let mut dir_ch = unbounded_channel::<Dir>();

    let floor = Arc::new(AtomicU64::new(args.minsize));
    let floor_clone = Arc::clone(&floor);

    let start_time = Instant::now();

    let t = thread::spawn(move || print_files(args.nentries, floor_clone, file_ch.1));

    let mut scans = vec![tokio::spawn(scan_dir(
        args.path,
        args.minsize,
        file_ch.0,
        dir_ch.0,
    ))];

    let mut dirs: usize = 1;
    while let Some(dir) = dir_ch.1.recv().await {
        scans.push(tokio::spawn(scan_dir(
            dir.path,
            floor.load(SeqCst),
            dir.tx_file,
            dir.tx_dir,
        )));
        dirs += 1;
    }

    let file_counts = join_all(scans).await;
    t.join().expect("failed to complete printer thread");
    let file_count: usize = file_counts.iter().map(|i| i.as_ref().unwrap().0).sum();
    let error_count: usize = file_counts.into_iter().map(|i| i.unwrap().1).sum();
    print_footer(start_time, file_count, error_count, dirs);
}
