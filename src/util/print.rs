use std::io;
use crossterm::cursor::{position, MoveTo, MoveToNextLine, MoveUp};
use crossterm::terminal::{Clear, ClearType, ScrollUp};
use crossterm::{
    execute,
    style::Print,
    style::{Attribute, Color, ResetColor, SetAttribute, SetForegroundColor},
};
use num_format::{Locale, ToFormattedString};
use std::io::stdout;
use std::time::SystemTime;
use chrono::{DateTime, Utc};
use tokio::time::Instant;

pub struct FilePrinter {
    max_line: u16,
    start_pos: (u16, u16),
}

impl FilePrinter {
    pub fn new(strap_line: &str) -> Self {
        execute!(
            stdout(),
            ScrollUp(12),
            MoveUp(12),
            SetForegroundColor(Color::Yellow),
            Print(strap_line),
            MoveToNextLine(2),
            Print(format!(
                "    {}{}Size(bytes)   created     modified    accessed     path",
                Attribute::Italic,
                Attribute::Underlined
            )),
            SetAttribute(Attribute::Reset),
            MoveToNextLine(1),
            ResetColor
        )
        .unwrap();
        Self {
            max_line: 0,
            start_pos: position().unwrap(),
        }
    }

    pub fn close(&self) {
        execute!(stdout(), MoveTo(0, self.max_line + 2)).unwrap();
    }

    pub fn print_line(&mut self, line: String, line_no: u16) {
        let _line_no = self.start_pos.1 + line_no;
        execute!(
            stdout(),
            MoveTo(0, _line_no),
            Print(line),
            Clear(ClearType::UntilNewLine),
        )
        .unwrap();
        self.max_line = _line_no.max(self.max_line);
    }
}

pub fn print_footer(start_time: Instant, file_count: usize, error_count: usize, dir_count: usize) {
    let end_time = Instant::now();
    let formatted_count = file_count.to_formatted_string(&Locale::en);
    let formatted_error_count = error_count.to_formatted_string(&Locale::en);
    let formatted_dir_count = dir_count.to_formatted_string(&Locale::en);
    let elapsed_time = end_time - start_time;
    execute!(
        stdout(),
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
        ScrollUp(1),
        MoveToNextLine(1),
        Print("file loading errors: "),
        SetForegroundColor(Color::Red),
        Print(formatted_error_count),
        ResetColor
    )
    .unwrap();
}


pub(crate) fn display_time(sys_time: io::Result<SystemTime>) -> String {
    if let Ok(t) = sys_time {
        let datetime: DateTime<Utc> = t.into();
        datetime.format("%Y-%m-%d").to_string()
    } else {
        return "-".into()
    }
}