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
}

impl FilePrinter {
    pub fn new(_strap_line: &str) -> Self {
        let args = Args::parse_args();
        let mut size_factor: f64 = 1f64;
        if args.g_byt {
            size_factor = 1024f64.powi(3);
        } else if args.m_byt {
            size_factor = 1024f64.powi(2);
        };

        let content_height = args.nentries.min(20) as u16;
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
        }
    }

    pub fn draw(
        &mut self,
        entries: &[Reverse<Filesize>],
        status: &ScanResult,
        elapsed: Option<Duration>,
    ) {
        let size_factor = self.size_factor;

        self.terminal
            .draw(|f| {
                let chunks = Layout::default()
                    .constraints([Constraint::Min(0), Constraint::Length(1)])
                    .split(f.area());

                let header_cells = ["Size", "Created", "Modified", "Used", "Path"]
                    .iter()
                    .enumerate()
                    .map(|(i, h)| {
                        if i == 0 {
                            Cell::from(Line::from(*h).alignment(Alignment::Right))
                                .style(Style::default().fg(Color::Yellow))
                        } else {
                            Cell::from(*h).style(Style::default().fg(Color::Yellow))
                        }
                    });
                let header = Row::new(header_cells).height(1).bottom_margin(1);

                let rows = entries.iter().map(|entry| {
                    let file = &entry.0;
                    let size_str = if size_factor == 1.0 {
                        file.size.to_formatted_string(&Locale::en)
                    } else {
                        format!("{:.3}", (file.size as f64) / size_factor)
                    };

                    let cells = vec![
                        Cell::from(Line::from(size_str).alignment(Alignment::Right)),
                        Cell::from(file.created.clone()),
                        Cell::from(file.modified.clone()),
                        Cell::from(file.used.clone()),
                        Cell::from(file.path.clone()),
                    ];
                    Row::new(cells).height(1)
                });

                let table = Table::new(
                    rows,
                    [
                        Constraint::Length(15),
                        Constraint::Length(12),
                        Constraint::Length(12),
                        Constraint::Length(12),
                        Constraint::Min(10),
                    ],
                )
                .header(header)
                .block(Block::default().borders(Borders::ALL).title("Scan Results"))
                .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

                f.render_stateful_widget(table, chunks[0], &mut self.table_state);

                let status_text = if let Some(dur) = elapsed {
                    format!(
                        "Done. Scanned: {} files, {} dirs, {} errors in {:.3}s.",
                        status.files.to_formatted_string(&Locale::en),
                        status.directories.to_formatted_string(&Locale::en),
                        status.errors.to_formatted_string(&Locale::en),
                        dur.as_secs_f64()
                    )
                } else {
                    format!(
                        "Scanning... Files: {}, Dirs: {}, Errors: {}",
                        status.files.to_formatted_string(&Locale::en),
                        status.directories.to_formatted_string(&Locale::en),
                        status.errors.to_formatted_string(&Locale::en)
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
