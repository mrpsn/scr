use crate::args::Args;
use crate::{Filesize, ScanResult};
use chrono::{DateTime, Utc};
use num_format::{Locale, ToFormattedString};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Terminal, TerminalOptions, Viewport,
};
use std::cmp::Reverse;
use std::io::{self, stdout, Stdout};
use std::time::{Duration, SystemTime};

pub struct FilePrinter {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    pub page_size: usize,
    pub table_state: TableState,
    size_factor: f64,
    size_heading: String,
    print_index: bool,
}

impl FilePrinter {
    pub fn new(_strap_line: &str) -> Self {
        let args = Args::parse_args();
        let mut size_factor: f64 = 1f64;
        let mut size_heading = "Size".to_string();

        if args.g_byt {
            size_factor = 1024f64.powi(3);
            size_heading = "Size (Gb)".to_string();
        } else if args.m_byt {
            size_factor = 1024f64.powi(2);
            size_heading = "Size (Mb)".to_string();
        };

        let content_height = args.nentries as u16;
        let height = content_height + 4;

        let backend = CrosstermBackend::new(stdout());
        let terminal = Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(height),
            },
        )
        .expect("Failed to init terminal");

        Self {
            terminal,
            page_size: args.nentries,
            table_state: TableState::default(),
            size_factor,
            size_heading,
            print_index: args.index_print,
        }
    }

    pub fn draw(
        &mut self,
        entries: &[Reverse<Filesize>],
        status: &ScanResult,
        elapsed: Option<Duration>,
    ) {
        let size_factor = self.size_factor;
        let size_heading = self.size_heading.as_str();
        let print_index = self.print_index;

        self.terminal
            .draw(|f| {
                let chunks = Layout::default()
                    .constraints([Constraint::Min(0), Constraint::Length(1)])
                    .split(f.area());

                let mut header_titles = if print_index { vec!["#"] } else { vec![] };
                header_titles.extend_from_slice(&[
                    size_heading,
                    "Created",
                    "Modified",
                    "Used",
                    "Path",
                ]);

                let header_cells = header_titles.into_iter().enumerate().map(|(i, h)| {
                    let is_size_column = if print_index { i == 1 } else { i == 0 };
                    if is_size_column {
                        Cell::from(Line::from(h).alignment(Alignment::Right))
                            .style(Style::default().fg(Color::Yellow))
                    } else {
                        Cell::from(h).style(Style::default().fg(Color::Yellow))
                    }
                });
                let header = Row::new(header_cells).height(1).bottom_margin(1);

                let rows = entries.iter().enumerate().map(|(idx, entry)| {
                    let file = &entry.0;
                    let size_str = if size_factor == 1.0 {
                        file.size.to_formatted_string(&Locale::en)
                    } else {
                        let val = (file.size as f64) / size_factor;
                        let s = format!("{:.3}", val);
                        if let Some((int_part, frac_part)) = s.split_once('.') {
                            if let Ok(n) = int_part.parse::<u64>() {
                                format!("{}.{}", n.to_formatted_string(&Locale::en), frac_part)
                            } else {
                                s
                            }
                        } else {
                            s
                        }
                    };

                    let mut cells = Vec::new();
                    if print_index {
                        cells.push(Cell::from((idx + 1).to_string()));
                    }

                    cells.extend_from_slice(&[
                        Cell::from(Line::from(size_str).alignment(Alignment::Right)),
                        Cell::from(file.created.clone()),
                        Cell::from(file.modified.clone()),
                        Cell::from(file.used.clone()),
                        Cell::from(file.path.clone()),
                    ]);
                    Row::new(cells).height(1)
                });

                let mut constraints = if print_index {
                    vec![Constraint::Length(5)]
                } else {
                    vec![]
                };
                constraints.extend_from_slice(&[
                    Constraint::Length(15),
                    Constraint::Length(12),
                    Constraint::Length(12),
                    Constraint::Length(12),
                    Constraint::Min(10),
                ]);

                let table = Table::new(rows, constraints)
                    .header(header)
                    .block(Block::default().borders(Borders::ALL).title("Scan Results"))
                    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

                f.render_stateful_widget(table, chunks[0], &mut self.table_state);

                let status_text = if let Some(dur) = elapsed {
                    let mut error_text = status.errors.to_formatted_string(&Locale::en);
                    if status.permission_denied > 0 {
                        error_text = format!(
                            "{} ({} permission blocked)",
                            error_text,
                            status.permission_denied.to_formatted_string(&Locale::en)
                        );
                    }

                    format!(
                        "Done. Scanned: {} files, {} dirs, {} errors in {:.3}s.",
                        status.files.to_formatted_string(&Locale::en),
                        status.directories.to_formatted_string(&Locale::en),
                        error_text,
                        dur.as_secs_f64()
                    )
                } else {
                    let mut error_text = status.errors.to_formatted_string(&Locale::en);
                    if status.permission_denied > 0 {
                        error_text = format!(
                            "{} ({} permission blocked)",
                            error_text,
                            status.permission_denied.to_formatted_string(&Locale::en)
                        );
                    }

                    format!(
                        "Scanning... Files: {}, Dirs: {}, Errors: {}",
                        status.files.to_formatted_string(&Locale::en),
                        status.directories.to_formatted_string(&Locale::en),
                        error_text
                    )
                };

                f.render_widget(Paragraph::new(status_text), chunks[1]);
            })
            .unwrap();
    }

    pub fn print_final(
        &mut self,
        entries: &[Reverse<Filesize>],
        status: &ScanResult,
        elapsed: Duration,
    ) {
        self.draw(entries, status, Some(elapsed));
        println!();
    }
}

pub fn display_time(sys_time: io::Result<SystemTime>) -> String {
    if let Ok(t) = sys_time {
        let datetime: DateTime<Utc> = t.into();
        datetime.format("%Y-%m-%d").to_string()
    } else {
        return "-".into();
    }
}
