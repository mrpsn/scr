use crate::args::Args;
use crate::{Filesize, ScanResult};
use chrono::{DateTime, Utc};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    style::Stylize,
    terminal::{
        self, disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use num_format::{Locale, ToFormattedString};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Terminal, TerminalOptions, Viewport,
};
use std::io::{self, stdout, IsTerminal, Stdout};
use std::time::{Duration, SystemTime};

pub struct FilePrinter {
    terminal: Option<Terminal<CrosstermBackend<Stdout>>>,
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

        let is_tty = stdout().is_terminal();
        let terminal = if is_tty {
            let content_height = args.nentries as u16;
            let height = content_height + 4;

            let backend = CrosstermBackend::new(stdout());
            Some(
                Terminal::with_options(
                    backend,
                    TerminalOptions {
                        viewport: Viewport::Inline(height),
                    },
                )
                .expect("Failed to init terminal"),
            )
        } else {
            None
        };

        Self {
            terminal,
            page_size: args.nentries,
            table_state: TableState::default(),
            size_factor,
            size_heading,
            print_index: args.index_print,
        }
    }

    fn format_size_static(size: u64, size_factor: f64) -> String {
        if size_factor == 1.0 {
            size.to_formatted_string(&Locale::en)
        } else {
            let val = (size as f64) / size_factor;
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
        }
    }

    pub fn draw(
        &mut self,
        entries: &[Filesize],
        status: &ScanResult,
        elapsed: Option<Duration>,
    ) {
        if let Some(terminal) = &mut self.terminal {
            let size_factor = self.size_factor;
            let size_heading = self.size_heading.clone();
            let print_index = self.print_index;
            let table_state = &mut self.table_state;

            terminal
                .draw(|f| {
                    let chunks = Layout::default()
                        .constraints([Constraint::Min(0), Constraint::Length(1)])
                        .split(f.area());

                    let mut header_titles = if print_index { vec!["#"] } else { vec![] };
                    header_titles.extend_from_slice(&[
                        size_heading.as_str(),
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
                        let file = entry;
                        let size_str = Self::format_size_static(file.size, size_factor);

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

                    f.render_stateful_widget(table, chunks[0], table_state);

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
    }

    pub fn print_final(
        &mut self,
        entries: &[Filesize],
        status: &ScanResult,
        elapsed: Duration,
    ) {
        // Clear and drop the terminal interface to return to standard stdout mode
        if let Some(mut terminal) = self.terminal.take() {
            let _ = terminal.clear();
        }

        let (_, term_h) = terminal::size().unwrap_or((80, 24));
        let required_height = entries.len() + 8; // Header(1) + Spacer(1) + Border(2) + Footer + DiskStats + Padding

        if (required_height as u16) > term_h {
            self.run_interactive_mode(entries, status, elapsed);
        } else {
            self.print_static_table(entries, status, elapsed);
        }
    }

    fn run_interactive_mode(
        &mut self,
        entries: &[Filesize],
        status: &ScanResult,
        elapsed: Duration,
    ) {
        // Setup for interactive mode
        enable_raw_mode().unwrap();
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen).unwrap();

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).unwrap();

        // Main Loop
        loop {
            let size_factor = self.size_factor;
            let size_heading = self.size_heading.clone();
            let print_index = self.print_index;
            let table_state = &mut self.table_state;

            terminal
                .draw(|f| {
                    let chunks = Layout::default()
                        .constraints([Constraint::Min(0), Constraint::Length(1)])
                        .split(f.area());

                    let mut header_titles = if print_index { vec!["#"] } else { vec![] };
                    header_titles.extend_from_slice(&[
                        size_heading.as_str(),
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
                        let file = entry;
                        let size_str = Self::format_size_static(file.size, size_factor);

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
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title("Scan Results (Press 'q' to quit)"),
                        )
                        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

                    f.render_stateful_widget(table, chunks[0], table_state);

                    // Render footer
                    let mut error_text = status.errors.to_formatted_string(&Locale::en);
                    if status.permission_denied > 0 {
                        error_text = format!(
                            "{} ({} permission blocked)",
                            error_text,
                            status.permission_denied.to_formatted_string(&Locale::en)
                        );
                    }
                    let status_text = format!(
                        "Done. Scanned: {} files, {} dirs, {} errors in {:.3}s.",
                        status.files.to_formatted_string(&Locale::en),
                        status.directories.to_formatted_string(&Locale::en),
                        error_text,
                        elapsed.as_secs_f64()
                    );
                    f.render_widget(Paragraph::new(status_text), chunks[1]);
                })
                .unwrap();

            if event::poll(Duration::from_millis(100)).unwrap() {
                if let Event::Key(key) = event::read().unwrap() {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => break,
                            KeyCode::Down | KeyCode::Char('j') => {
                                let i = match self.table_state.selected() {
                                    Some(i) => {
                                        if i >= entries.len() - 1 {
                                            0
                                        } else {
                                            i + 1
                                        }
                                    }
                                    None => 0,
                                };
                                self.table_state.select(Some(i));
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                let i = match self.table_state.selected() {
                                    Some(i) => {
                                        if i == 0 {
                                            entries.len() - 1
                                        } else {
                                            i - 1
                                        }
                                    }
                                    None => 0,
                                };
                                self.table_state.select(Some(i));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // Restore terminal
        disable_raw_mode().unwrap();
        execute!(terminal.backend_mut(), LeaveAlternateScreen,).unwrap();
        terminal.show_cursor().unwrap();
    }

    fn print_static_table(
        &mut self,
        entries: &[Filesize],
        status: &ScanResult,
        elapsed: Duration,
    ) {
        let (term_w, _) = terminal::size().unwrap_or((80, 24));
        let term_w = term_w as usize;

        // Define column widths
        let idx_w = if self.print_index { 6 } else { 0 };
        let size_w = 15;
        let date_w = 12;

        let mut fixed_w = 1; // Left border
        if self.print_index {
            fixed_w += idx_w + 2;
        }
        fixed_w += size_w + 2;
        fixed_w += date_w + 2;
        fixed_w += date_w + 2;
        fixed_w += date_w + 2;
        fixed_w += 1; // Right border

        let path_w = if term_w > fixed_w + 5 {
            term_w - fixed_w
        } else {
            10
        };

        // Top Border
        let title = "Scan Results";
        let mut top_line = String::with_capacity(term_w);
        top_line.push('┌');
        top_line.push_str(title);
        for _ in 0..term_w.saturating_sub(2 + title.len()) {
            top_line.push('─');
        }
        top_line.push('┐');
        println!("{}", top_line);

        // Header
        print!("│");
        if self.print_index {
            print!("{}  ", format!("{:>width$}", "#", width = idx_w).yellow());
        }
        print!(
            "{}  ",
            format!("{:>width$}", self.size_heading.as_str(), width = size_w).yellow()
        );
        print!("{}  ", format!("{:<width$}", "Created", width = date_w).yellow());
        print!("{}  ", format!("{:<width$}", "Modified", width = date_w).yellow());
        print!("{}  ", format!("{:<width$}", "Used", width = date_w).yellow());
        print!("{}", format!("{:<width$}", "Path", width = path_w).yellow());
        println!("│");

        // Spacer
        print!("│");
        for _ in 0..term_w.saturating_sub(2) {
            print!(" ");
        }
        println!("│");

        // Rows
        for (i, entry) in entries.iter().enumerate() {
            let file = entry;
            let size_str = Self::format_size_static(file.size, self.size_factor);

            print!("│");
            if self.print_index {
                print!("{:>width$}  ", i + 1, width = idx_w);
            }
            print!("{:>width$}  ", size_str, width = size_w);
            print!("{:<width$}  ", file.created, width = date_w);
            print!("{:<width$}  ", file.modified, width = date_w);
            print!("{:<width$}  ", file.used, width = date_w);

            let p_chars: Vec<char> = file.path.chars().collect();
            if p_chars.len() > path_w {
                let s: String = p_chars.into_iter().take(path_w).collect();
                print!("{}", s);
            } else {
                print!("{:<width$}", file.path, width = path_w);
            }
            println!("│");
        }

        // Bottom Border
        let mut bot_line = String::new();
        bot_line.push('└');
        for _ in 0..term_w.saturating_sub(2) {
            bot_line.push('─');
        }
        bot_line.push('┘');
        println!("{}", bot_line);

        // Footer status
        let mut error_text = status.errors.to_formatted_string(&Locale::en);
        if status.permission_denied > 0 {
            error_text = format!(
                "{} ({} permission blocked)",
                error_text,
                status.permission_denied.to_formatted_string(&Locale::en)
            );
        }

        println!(
            "Done. Scanned: {} files, {} dirs, {} errors in {:.3}s.",
            status.files.to_formatted_string(&Locale::en),
            status.directories.to_formatted_string(&Locale::en),
            error_text,
            elapsed.as_secs_f64()
        );

        // Calculate and print disk usage
        let args = Args::parse_args();
        let disks = Disks::new_with_refreshed_list();

        // Find the disk containing the path
        // We look for the longest mount point that is a prefix of our path
        let mut best_match: Option<&sysinfo::Disk> = None;
        let mut best_len = 0;

        // Normalize path to absolute if possible to match mount points better
        let abs_path = std::fs::canonicalize(&args.path).unwrap_or(args.path.clone());

        for disk in &disks {
            if let Some(mount) = disk.mount_point().to_str() {
                if abs_path.to_string_lossy().starts_with(mount) && mount.len() > best_len {
                    best_len = mount.len();
                    best_match = Some(disk);
                }
            }
        }

        if let Some(disk) = best_match {
            let total = disk.total_space();
            let available = disk.available_space();
            let used = total - available;
            let percent = if total > 0 {
                (used as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            let total_str = Self::format_size_static(total, self.size_factor);

            println!(
                "Total Disk Size: {} {}, Used: {:.2}%",
                total_str,
                self.size_heading
                    .replace("Size ", "")
                    .replace("(", "")
                    .replace(")", ""),
                percent
            );
        }
    }
}

use sysinfo::Disks;

pub fn display_time(sys_time: io::Result<SystemTime>) -> String {
    if let Ok(t) = sys_time {
        let datetime: DateTime<Utc> = t.into();
        return datetime.format("%Y-%m-%d").to_string();
    }
    "-".into()
}
