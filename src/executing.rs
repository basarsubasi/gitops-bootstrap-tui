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
                    let parts: Vec<&str> = chunk.split('\n').collect();
                    for (i, part) in parts.iter().enumerate() {
                        if i == 0 {
                            if let Some(last) = self.logs.last_mut() {
                                last.push_str(part);
                            } else {
                                self.logs.push(part.to_string());
                            }
                        } else {
                            self.logs.push(part.to_string());
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
        
        let mut text = String::new();
        for log in &self.logs {
            text.push_str(log);
            text.push('\n');
        }

        if let Some(ref err) = self.error {
            text.push_str("\n\x1b[1;31m[ERROR]\x1b[0m\n");
            text.push_str(err);
            text.push_str("\n\nPress ESC or 'b' to go back and retry.");
        } else if self.is_done {
            text.push_str("\n\x1b[1;32m[SUCCESS]\x1b[0m\n");
            text.push_str("All operations completed successfully!\n\nPress Enter or 'q' to quit.");
        } else {
            text.push_str("\n\x1b[1;33m[RUNNING...]\x1b[0m\n");
        }

        // Auto-scroll to bottom
        let num_lines = text.lines().count();
        let height = area.height as usize;
        let scroll = if num_lines > height {
            (num_lines - height + 2) as u16
        } else {
            0
        };

        let p = Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: true })
            .scroll((scroll, 0));

        f.render_widget(p, area);
    }
}
