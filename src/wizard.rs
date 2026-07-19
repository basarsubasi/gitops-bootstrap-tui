use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph},
};
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;

#[derive(PartialEq)]
pub enum WizardAction {
    None,
    Next,
    Previous,
}

pub struct WizardState {
    pub inputs: Vec<Input>,
    pub labels: Vec<&'static str>,
    pub current_input: usize,
    pub action: WizardAction,
    pub error_message: Option<String>,
}

impl WizardState {
    pub fn new(config: &crate::config::AppConfig) -> Self {
        let repo_url = config.template_repo_url.clone();
        let base_dir = config.base_dir_path.clone();
        let cluster_name = config.new_cluster_name.clone();
        let gitops_dir = config.gitops_dir_path.clone();

        let inputs = vec![
            Input::default()
                .with_value(repo_url.clone())
                .with_cursor(repo_url.chars().count()),
            Input::default()
                .with_value(base_dir.clone())
                .with_cursor(base_dir.chars().count()),
            Input::default()
                .with_value(cluster_name.clone())
                .with_cursor(cluster_name.chars().count()),
            Input::default()
                .with_value(gitops_dir.clone())
                .with_cursor(gitops_dir.chars().count()),
        ];
        let labels = vec![
            "Template Git Repo URL",
            "Base Directory",
            "New Cluster Name",
            "GitOps Output Directory Path",
        ];
        Self {
            inputs,
            labels,
            current_input: 0,
            action: WizardAction::None,
            error_message: None,
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> bool {
        if let Event::Key(key) = event {
            if self.error_message.is_some() {
                self.error_message = None;
                return false;
            }

            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                return true; // Should quit
            }

            match key.code {
                KeyCode::Up | KeyCode::BackTab => {
                    if self.current_input > 0 {
                        self.current_input -= 1;
                    } else {
                        self.current_input = self.inputs.len() + 1; // Wrap to Next button
                    }
                }
                KeyCode::Down | KeyCode::Tab => {
                    if self.current_input < self.inputs.len() + 1 {
                        self.current_input += 1;
                    } else {
                        self.current_input = 0; // Wrap to first input
                    }
                }
                KeyCode::Left | KeyCode::Right => {
                    if self.current_input >= self.inputs.len() {
                        if self.current_input == self.inputs.len() {
                            self.current_input = self.inputs.len() + 1;
                        } else {
                            self.current_input = self.inputs.len();
                        }
                    } else {
                        self.inputs[self.current_input].handle_event(event);
                    }
                }
                KeyCode::Enter => {
                    if self.current_input == self.inputs.len() {
                        self.action = WizardAction::Next;
                    } else if self.current_input == self.inputs.len() + 1 {
                        self.action = WizardAction::Previous;
                    } else {
                        self.current_input += 1;
                    }
                }
                _ => {
                    if self.current_input < self.inputs.len() {
                        self.inputs[self.current_input].handle_event(event);
                    }
                }
            }
        }
        false
    }

    pub fn render(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let mut constraints = vec![];
        for _ in 0..self.labels.len() {
            constraints.push(Constraint::Length(4)); // Give more vertical space!
        }
        constraints.push(Constraint::Length(1)); // Space for error message
        constraints.push(Constraint::Length(3)); // Buttons
        constraints.push(Constraint::Min(0));

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints(constraints)
            .split(area);

        for (i, label) in self.labels.iter().enumerate() {
            let style = if i == self.current_input && self.action == WizardAction::None {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let val = self.inputs[i].value();
            let display_text = if val.is_empty() {
                if i == self.current_input && self.action == WizardAction::None {
                    "Type here..."
                } else {
                    "(empty)"
                }
            } else {
                val
            };

            let line = if i == self.current_input && self.action == WizardAction::None {
                let cursor = self.inputs[i].cursor();
                let mut before = String::new();
                let mut cursor_char = String::from(" ");
                let mut after = String::new();

                for (idx, c) in display_text.chars().enumerate() {
                    if idx < cursor {
                        before.push(c);
                    } else if idx == cursor {
                        cursor_char = c.to_string();
                    } else {
                        after.push(c);
                    }
                }

                ratatui::text::Line::from(vec![
                    Span::styled(before, style),
                    Span::styled(
                        cursor_char,
                        Style::default().bg(Color::White).fg(Color::Black),
                    ),
                    Span::styled(after, style),
                ])
            } else {
                ratatui::text::Line::from(Span::styled(display_text, style))
            };

            let p = Paragraph::new(line)
                .wrap(ratatui::widgets::Wrap { trim: false })
                .block(
                    Block::default()
                        .title(*label)
                        .borders(Borders::ALL)
                        .border_style(style),
                );
            f.render_widget(p, chunks[i]);
        }

        if let Some(err) = &self.error_message {
            let p = Paragraph::new(Span::styled(err, Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)))
                .alignment(ratatui::layout::Alignment::Center);
            f.render_widget(p, chunks[self.inputs.len()]);
        }

        // Render Buttons
        let next_idx = self.inputs.len();
        let prev_idx = self.inputs.len() + 1;

        let prev_style = if self.current_input == prev_idx {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        };

        let next_style = if self.current_input == next_idx {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        };

        let btn_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(30),
                Constraint::Percentage(30),
                Constraint::Percentage(20),
            ])
            .split(chunks[chunks.len() - 2]);

        let prev_btn = Paragraph::new(Span::raw("   [ PREVIOUS ]   "))
            .style(prev_style)
            .alignment(ratatui::layout::Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(prev_style),
            );

        let next_btn = Paragraph::new(Span::raw("   [ NEXT ]   "))
            .style(next_style)
            .alignment(ratatui::layout::Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(next_style),
            );

        // Move previous to right
        f.render_widget(next_btn, btn_chunks[1]);
        f.render_widget(prev_btn, btn_chunks[2]);
    }
}
