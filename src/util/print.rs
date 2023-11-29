use std::fmt::{Display, Formatter};
use std::io;
use crossterm::cursor::{position, MoveTo};
use crossterm::terminal::{Clear, ClearType, ScrollUp};
use crossterm::{execute, style::Print, style::{Attribute, Color, ResetColor, SetAttribute, SetForegroundColor}, terminal, queue};
use num_format::{Locale, ToFormattedString};
use std::io::{stdout, Write};
use std::time::{SystemTime};
use chrono::{DateTime, Utc};
use sorted_vec::ReverseSortedVec;
use crate::{Filesize, ScanResult, StatusMsg};
use crate::args::Args;


struct Status<'a>(&'a ScanResult);

impl<'a> Display for Status<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let errors: String = match self.0.errors > 0 {
            true => format!("errors: {}", self.0.errors.to_formatted_string(&Locale::en)),
            false => "".into(),
        };
        write!(f, "scanned files: {} directories: {} {errors}",
               self.0.files.to_formatted_string(&Locale::en),
               self.0.directories.to_formatted_string(&Locale::en),
        )
    }
}


struct FileFormat<'a>(&'a Filesize, f64);
impl<'a> Display for FileFormat<'a>{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {

        let size_str = match self.1 == 1.0 {
            true => self.0.size.to_formatted_string(&Locale::en),
            _ => format!("{:.3}", (self.0.size as f64) / self.1),
        };

        write!(f, "{size_str:>15}  {:>10}  {:>10}  {:>10}  {}",
               self.0.created, self.0.modified, self.0.used, self.0.path
        )
    }
}

#[derive(Clone)]
pub struct FilePrinter {
    max_line: u16,
    status_line: u16,
    start_line: i16,
    pub page_size: usize,
    print_index: bool,
    size_factor: f64,
    flush_count: usize,
    status_count: usize,
}

impl FilePrinter {
    pub fn print_status(&mut self, msg: StatusMsg) {
        self.status_count += 1;
        queue!(stdout(), MoveTo(0, self.status_line)).unwrap();

        match msg {
            StatusMsg::Final(sr, elapsed_time) => {
                queue!(
                    stdout(),
                    Print(Status(&sr)),
                    Print(" in "),
                    SetForegroundColor(Color::Green),
                    Print(format!("{:.3}", elapsed_time.as_secs_f64())),
                    ResetColor,
                    Print(" seconds"),
                ).unwrap();
            },
            StatusMsg::Status(sr) => queue!(stdout(), Print(Status(sr))).unwrap(),
        }
        if self.status_count % 20 == 0 {
            stdout().flush().unwrap();
        }
    }

    pub fn new(_strap_line: &str) -> Self {
        let args = Args::parse_args();

        let mut size_factor: f64 = 1f64;
        let mut size_heading: String = "Bytes".into();
        if args.g_byt {
            size_factor = 1024f64.powi(3);
            size_heading = "Gb".into();
        } else if args.m_byt {
            size_factor = 1024f64.powi(2);
            size_heading = "Mb".into();
        };

        let lpad = match args.index_print {
            true => "    ",
            false => "",
        };
        execute!(
            stdout(),
            ScrollUp(2),
            SetForegroundColor(Color::Yellow),
            Print("\n"),
            Print(format!(
                "{lpad}{}{}    {size_heading:>10}    created     modified    accessed     path",
                Attribute::Italic,
                Attribute::Underdotted,
            )),
            SetAttribute(Attribute::Reset),
            Print("\n"),
            ResetColor
        ).unwrap();

        let pos = position().unwrap().1 as i16;
        Self {
            max_line: 0,
            status_line: position().unwrap().1 - 3,
            start_line: pos,
            page_size: 30,
            print_index: args.index_print,
            size_factor,
            flush_count: 0,
            status_count: 0,
        }
    }

    pub fn print_line(&mut self, entry: &Filesize, line_no: usize) {
        if line_no < self.page_size {
            self.print( entry, line_no)
        }
    }

    pub fn print_final(mut self, entries: ReverseSortedVec<Filesize>, status: StatusMsg) {
        let lines = self.page_size;
        self.print_status(status);

        if entries.len() > lines {
            queue!(stdout(), MoveTo(0, self.max_line)).unwrap();

            for (i, entry) in entries.iter().skip(lines).enumerate() {
                let ff = FileFormat(&entry.0, self.size_factor);
                let (_, scrolls) = print(ff, lines + i, self.start_line, self.print_index, false);
                queue!(stdout(), Print("\n")).unwrap();
                self.status_line -= scrolls;
            }

        } else {
            queue!(stdout(), MoveTo(0, self.max_line)).unwrap();
        }
        execute!(stdout(), Print("\n\n")).unwrap();
    }

    fn print(&mut self, entry: &Filesize, line_no: usize) {
        self.flush_count += 1;
        let ff = FileFormat(entry, self.size_factor);
        let (_line_no, scrolls) = print(ff, line_no, self.start_line, self.print_index, self.flush_count % 2 ==0);
        self.max_line = _line_no.max(self.max_line);
        self.start_line -= scrolls as i16;
        self.status_line -= scrolls;
    }
}


pub fn display_time(sys_time: io::Result<SystemTime>) -> String {
    if let Ok(t) = sys_time {
        let datetime: DateTime<Utc> = t.into();
        datetime.format("%Y-%m-%d").to_string()
    } else {
        return "-".into()
    }
}

fn print(entry: FileFormat, line_no: usize, start_line: i16, print_index: bool, flush: bool) -> (u16, u16) {
    let mut _line_no = (start_line + line_no as i16) as u16;
    let terminal_end = terminal::size().unwrap().1;
    let mut scrolls: u16 = 0;
    if _line_no == terminal_end {
        queue!(
                    stdout(),
                    ScrollUp(1),
                    MoveTo(0, terminal_end),
                ).unwrap();
        _line_no = terminal_end - 1;
        scrolls = 1;
    }

    queue!(
                stdout(),
                MoveTo(0, _line_no),
                Print(if print_index {format!("{:>3} ", line_no + 1)}  else {"".into()}),
                Print(entry),
                Clear(ClearType::UntilNewLine),
            )
        .unwrap();
    if flush {
        stdout().flush().unwrap();
    }
    (_line_no, scrolls)
}