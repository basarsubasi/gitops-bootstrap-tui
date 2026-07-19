use crossterm::event::{Event, KeyCode};
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Paragraph, Wrap},
};
use std::sync::mpsc::Receiver;

pub enum ExecutionEvent {
    Log(String),
    Error(String),
    Done,
}

pub struct ExecutingState {
    pub logs: Vec<String>,
    pub error: Option<String>,
    pub is_done: bool,
    pub rx: Receiver<ExecutionEvent>,
    pub go_back: bool,
    pub should_quit: bool,
}

impl ExecutingState {
    pub fn new(rx: Receiver<ExecutionEvent>) -> Self {
        Self {
            logs: vec![],
            error: None,
            is_done: false,
            rx,
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
            } else if self.is_done
                && (key.code == KeyCode::Enter || key.code == KeyCode::Char('q')) {
                    self.should_quit = true;
                    return true;
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
            text.push_str("\n[ERROR]\n");
            text.push_str(err);
            text.push_str("\n\nPress ESC or 'b' to go back and retry.");
        } else if self.is_done {
            text.push_str("\n[SUCCESS]\n");
            text.push_str("All operations completed successfully!\n\nPress Enter or 'q' to quit.");
        } else {
            text.push_str("\n[RUNNING...]\n");
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
