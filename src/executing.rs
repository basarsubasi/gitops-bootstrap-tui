use crossterm::event::{Event, KeyCode};
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Paragraph, Wrap},
};
use std::sync::mpsc::Receiver;

pub enum ExecutionEvent {
    Log(String),
    LogChunk(String),
    Error(String),
    Done,
}

pub struct ExecutingState {
    pub logs: Vec<String>,
    pub error: Option<String>,
    pub is_done: bool,
    pub rx: Receiver<ExecutionEvent>,
    pub input_tx: Option<std::sync::mpsc::Sender<String>>,
    pub go_back: bool,
    pub should_quit: bool,
}

impl ExecutingState {
    pub fn new(rx: Receiver<ExecutionEvent>, input_tx: Option<std::sync::mpsc::Sender<String>>) -> Self {
        Self {
            logs: vec![],
            error: None,
            is_done: false,
            rx,
            input_tx,
            go_back: false,
            should_quit: false,
        }
    }

    pub fn poll_events(&mut self) {
        if self.is_done || self.error.is_some() {
            return;
        }
        while let Ok(event) = self.rx.try_recv() {
            match event {
                ExecutionEvent::Log(msg) => self.logs.push(msg),
                ExecutionEvent::LogChunk(chunk) => {
                    let cleaned = String::from_utf8_lossy(
                        &strip_ansi_escapes::strip(chunk.as_bytes())
                    ).to_string();

                    for c in cleaned.chars() {
                        if c == '\n' {
                            self.logs.push(String::new());
                        } else if c == '\r' {
                            if let Some(last) = self.logs.last_mut() {
                                // If we get \r, just clear the current line so it redraws over it
                                last.clear();
                            } else {
                                self.logs.push(String::new());
                            }
                        } else {
                            if self.logs.is_empty() {
                                self.logs.push(String::new());
                            }
                            self.logs.last_mut().unwrap().push(c);
                        }
                    }
                }
                ExecutionEvent::Error(err) => self.error = Some(err),
                ExecutionEvent::Done => self.is_done = true,
            }
        }
    }

    pub fn handle_event(&mut self, ev: &Event) -> bool {
        if let Event::Key(key) = ev {
            if self.error.is_some() {
                if key.code == KeyCode::Esc || key.code == KeyCode::Char('b') {
                    self.go_back = true;
                    return true;
                }
            } else if self.is_done {
                if key.code == KeyCode::Enter || key.code == KeyCode::Char('q') {
                    self.should_quit = true;
                    return true;
                }
            } else {
                // Not done, not error: we are running. Capture keystrokes.
                if let Some(tx) = &self.input_tx {
                    match key.code {
                        KeyCode::Char(c) => {
                            let _ = tx.send(c.to_string());
                            // local echo
                            if let Some(last) = self.logs.last_mut() {
                                last.push(c);
                            }
                        }
                        KeyCode::Enter => {
                            let _ = tx.send("\n".to_string());
                            // local echo
                            self.logs.push("".to_string());
                        }
                        _ => {}
                    }
                }
            }
        }
        false
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        let block = Block::default().borders(Borders::ALL).title(" Execution Log ");
        
        let mut ui_lines = Vec::new();
        for log in &self.logs {
            if log.starts_with("ERROR:") || log.starts_with("Error") {
                ui_lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                    log, ratatui::style::Style::default().fg(ratatui::style::Color::Red).add_modifier(ratatui::style::Modifier::BOLD)
                )));
            } else if log.starts_with("✓") {
                ui_lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                    log, ratatui::style::Style::default().fg(ratatui::style::Color::Green).add_modifier(ratatui::style::Modifier::BOLD)
                )));
            } else if log.starts_with("[1/3]") || log.starts_with("[2/3]") || log.starts_with("[3/3]") {
                ui_lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                    log, ratatui::style::Style::default().fg(ratatui::style::Color::Cyan).add_modifier(ratatui::style::Modifier::BOLD)
                )));
            } else if let Some(idx) = log.find("[y/N] ") {
                let prefix = &log[..idx];
                let prompt = "[y/N] ";
                let user_input = &log[idx + prompt.len()..];
                
                ui_lines.push(ratatui::text::Line::from(vec![
                    ratatui::text::Span::raw(prefix),
                    ratatui::text::Span::styled(prompt, ratatui::style::Style::default().fg(ratatui::style::Color::Yellow).add_modifier(ratatui::style::Modifier::BOLD)),
                    ratatui::text::Span::styled(user_input, ratatui::style::Style::default().fg(ratatui::style::Color::Green).add_modifier(ratatui::style::Modifier::BOLD)),
                ]));
            } else {
                ui_lines.push(ratatui::text::Line::from(log.as_str()));
            }
        }

        if let Some(ref err) = self.error {
            ui_lines.push(ratatui::text::Line::from(""));
            ui_lines.push(ratatui::text::Line::from(ratatui::text::Span::styled("[ERROR]", ratatui::style::Style::default().fg(ratatui::style::Color::Red).add_modifier(ratatui::style::Modifier::BOLD))));
            ui_lines.push(ratatui::text::Line::from(err.as_str()));
            ui_lines.push(ratatui::text::Line::from(""));
            ui_lines.push(ratatui::text::Line::from("Press ESC or 'b' to go back and retry."));
        } else if self.is_done {
            ui_lines.push(ratatui::text::Line::from(""));
            ui_lines.push(ratatui::text::Line::from(ratatui::text::Span::styled("[SUCCESS]", ratatui::style::Style::default().fg(ratatui::style::Color::Green).add_modifier(ratatui::style::Modifier::BOLD))));
            ui_lines.push(ratatui::text::Line::from("All operations completed successfully!"));
            ui_lines.push(ratatui::text::Line::from(""));
            ui_lines.push(ratatui::text::Line::from("Press Enter or 'q' to quit."));
        } else {
            ui_lines.push(ratatui::text::Line::from(""));
            ui_lines.push(ratatui::text::Line::from(ratatui::text::Span::styled("[RUNNING...]", ratatui::style::Style::default().fg(ratatui::style::Color::Yellow).add_modifier(ratatui::style::Modifier::BOLD))));
        }

        // Auto-scroll to bottom by calculating actual wrapped physical lines
        let width = area.width.saturating_sub(2).max(1) as usize; // account for borders
        let mut physical_lines = 0;
        for line in &ui_lines {
            let line_width = line.width();
            if line_width == 0 {
                physical_lines += 1;
            } else {
                physical_lines += (line_width.saturating_sub(1) / width) + 1;
            }
        }

        let height = area.height.saturating_sub(2) as usize; // account for borders
        let scroll = if physical_lines > height {
            (physical_lines - height) as u16
        } else {
            0
        };

        let p = Paragraph::new(ui_lines)
            .block(block)
            .wrap(Wrap { trim: true })
            .scroll((scroll, 0));

        f.render_widget(p, area);
    }
}
