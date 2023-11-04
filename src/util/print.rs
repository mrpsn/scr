use crossterm::cursor::{position, MoveTo, MoveUp, MoveToNextLine};
use crossterm::{execute, style::Print, style::{SetForegroundColor, Color, ResetColor}};
use crossterm::terminal::{ScrollUp};
use std::io::{stdout};
use num_format::{Locale, ToFormattedString};
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
            MoveToNextLine(1),
            Print(" | Size(bytes) | created  | modified | accessed | path"),
            MoveToNextLine(1),
            ResetColor
        ).unwrap();
        Self {max_line: 0, start_pos: position().unwrap()}
    }

    pub fn close(&self) {
        println!("\x1B[{:};999H", self.max_line + 2);  // move to end
    }

    pub fn print_line(&mut self, line: String, line_no: u16) {
        let _line_no = self.start_pos.1 + line_no;
        execute!(
            stdout(),
            MoveTo(0, _line_no),
            Print(&line),
        ).unwrap();
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
    ).unwrap();
}
