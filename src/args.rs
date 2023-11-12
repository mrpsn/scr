use clap::Parser;
use std::path::PathBuf;

/// A fast directory tree scanner, listing the top n files in the tree
/// by size. Intended use, is to help quickly identify which files are
/// consuming space on your drive.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// A valid directory path to start scanning from. Defaults to '.'
    #[arg(index = 1, value_name = "PATH")]
    pub path: PathBuf,

    /// Find files >= to size (in bytes).
    #[arg(short, long, value_name = "MINSIZE", default_value_t = 0)]
    pub minsize: u64,

    /// number of entries to display
    #[arg(short, long, value_name = "N_ENTRIES", default_value_t = 10)]
    pub nentries: usize,
}
