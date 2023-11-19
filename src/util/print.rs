use std::fmt::{Display, Formatter};
use std::io;
use crossterm::cursor::{position, MoveTo, MoveToNextLine, MoveUp};
use crossterm::terminal::{Clear, ClearType, ScrollUp};
use crossterm::{execute, style::Print, style::{Attribute, Color, ResetColor, SetAttribute, SetForegroundColor}, terminal, queue};
use num_format::{Locale, ToFormattedString};
use std::io::{stdout, Write};
use std::time::{Duration, SystemTime};
use chrono::{DateTime, Utc};
use itertools::Itertools;
use crate::{Filesize, ScanResult};
use crate::args::Args;


struct FileFormat(Filesize, f64);
impl Display for FileFormat{
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

struct Line {
    entry: Filesize,
    number: u16,
}

pub struct FilePrinter {
    max_line: u16,
    start_line: i16,
    buffer: Vec<Line>,
    max_print: u16,
    print_index: bool,
    size_factor: f64,
}

impl FilePrinter {
    pub fn new(strap_line: &str) -> Self {
        let args = Args::parse_args();

        let mut size_factor: f64 = 1f64;
        let mut size_heading: String = "Byt".into();
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
            ScrollUp(12),
            MoveUp(12),
            SetForegroundColor(Color::Yellow),
            Print(strap_line),
            MoveToNextLine(2),
            Print(format!(
                "    {}{}{}  Size({})    created     modified    accessed     path",
                lpad,
                Attribute::Italic,
                Attribute::Underdotted,
                size_heading,
            )),
            SetAttribute(Attribute::Reset),
            MoveToNextLine(1),
            ResetColor
        )
            .unwrap();
        Self {
            max_line: 0,
            start_line: position().unwrap().1 as i16,
            buffer: vec![],
            max_print: 30,
            print_index: args.index_print,
            size_factor,
        }
    }

    pub fn print_line(&mut self, entry: Filesize, line_no: u16) {
        if line_no >= self.max_print {
            self.buffer.push(Line { entry, number: line_no })
        } else {
            self.print(entry, line_no)
        }
    }

    pub fn print_final(mut self, n: usize) {
        let limit = self.max_print as usize;

        if n > limit {
            execute!(stdout(), MoveTo(0, self.max_line)).unwrap();
            let to_print = n - limit;
            for (i, line) in self.buffer.into_iter().sorted_by_key(|l| l.number).rev().take(to_print).enumerate() {
                let line_number = (limit + i) as u16;
                let ff = FileFormat(line.entry, self.size_factor);
                let (_line_no, scrolls) = print(ff, line_number, self.start_line, self.print_index);
                self.max_line = _line_no.max(self.max_line);
                self.start_line -= scrolls;
            }
        }
        execute!(stdout(), MoveTo(0, self.max_line)).unwrap();
    }

    fn print(&mut self, entry: Filesize, line_no: u16) {
        let ff = FileFormat(entry, self.size_factor);
        let (_line_no, scrolls) = print(ff, line_no, self.start_line, self.print_index);
        self.max_line = _line_no.max(self.max_line);
        self.start_line -= scrolls;
    }
}


pub fn print_footer(elapsed_time: Duration, total: ScanResult) {

    let formatted_count = total.files.to_formatted_string(&Locale::en);
    let formatted_dir_count = total.directories.to_formatted_string(&Locale::en);

    let mut stdout = stdout();

    queue!(
        stdout,
        Print("\n"),
        MoveToNextLine(2),
        Print("scanned "),
        SetForegroundColor(Color::Green),
        Print(formatted_count),
        ResetColor,
        Print(" files, "),
        SetForegroundColor(Color::Green),
        Print(formatted_dir_count),
        ResetColor,
        Print(" directories in "),
        SetForegroundColor(Color::Green),
        Print(format!("{:.3}", elapsed_time.as_secs_f64())),
        ResetColor,
        Print(" seconds"),
    ).unwrap();

    if total.errors > 0 {
        queue!(
            stdout,
            Print(". (File loading errors: "),
            SetForegroundColor(Color::Red),
            Print(total.errors.to_formatted_string(&Locale::en)),
            ResetColor,
            Print(")"),
        ).unwrap();
    }
    queue!(stdout, Print("\n")).unwrap();
    stdout.flush().unwrap();
}


pub(crate) fn display_time(sys_time: io::Result<SystemTime>) -> String {
    if let Ok(t) = sys_time {
        let datetime: DateTime<Utc> = t.into();
        datetime.format("%Y-%m-%d").to_string()
    } else {
        return "-".into()
    }
}

fn print(entry: FileFormat, line_no: u16, start_line: i16, print_index: bool) -> (u16, i16) {
    let mut _line_no = (start_line + line_no as i16) as u16;
    let terminal_end = terminal::size().unwrap().1;
    let mut scrolls: i16 = 0;
    if _line_no == terminal_end {
        execute!(
                    stdout(),
                    ScrollUp(1),
                    MoveTo(0, terminal_end),
                ).unwrap();
        _line_no = terminal_end - 1;
        scrolls = 1;
    }

    execute!(
                stdout(),
                MoveTo(0, _line_no),
                Print(if print_index {format!("{:>3} ", line_no + 1)}  else {"".into()}),
                Print(entry),
                Clear(ClearType::UntilNewLine),
            )
        .unwrap();
    (_line_no, scrolls)
}