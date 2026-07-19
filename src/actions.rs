use crossterm::event::{Event, KeyCode};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;

#[derive(PartialEq, Clone, Copy)]
pub enum ActionsFocus {
    List,
    Previous,
    Next,
    ModalGit,
    ModalFlux,
}

pub struct ModalState {
    pub inputs: Vec<Input>,
    pub labels: Vec<&'static str>,
    pub title: &'static str,
    pub current_input: usize,
}

impl ModalState {
    pub fn new_flux(config: &crate::config::AppConfig) -> Self {
        let git_url = config.flux_git_url.clone();
        let git_branch = config.flux_git_branch.clone();
        let cluster_path = format!("./{}", config.new_cluster_name);
        let kubeconfig = config.flux_kubeconfig.clone();

        let inputs = vec![
            Input::default()
                .with_value(git_url.clone())
                .with_cursor(git_url.chars().count()),
            Input::default()
                .with_value(git_branch.clone())
                .with_cursor(git_branch.chars().count()),
            Input::default()
                .with_value(cluster_path.clone())
                .with_cursor(cluster_path.chars().count()),
            Input::default()
                .with_value(kubeconfig.clone())
                .with_cursor(kubeconfig.chars().count()),
        ];
        let labels = vec!["Git URL", "Git Branch", "Git Path", "Kubeconfig Path"];
        Self {
            inputs,
            labels,
            title: " FLUX CONFIGURATION ",
            current_input: 0,
        }
    }

    pub fn new_git(config: &crate::config::AppConfig) -> Self {
        let daemon_addr = config.git_daemon_address.clone();
        let git_branch = config.git_branch.clone();

        let inputs = vec![
            Input::default()
                .with_value(daemon_addr.clone())
                .with_cursor(daemon_addr.chars().count()),
            Input::default()
                .with_value(git_branch.clone())
                .with_cursor(git_branch.chars().count()),
        ];
        let labels = vec!["Git Daemon Listen Address", "Git Initial Branch"];
        Self {
            inputs,
            labels,
            title: " GIT DAEMON CONFIGURATION ",
            current_input: 0,
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> bool {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Up | KeyCode::BackTab => {
                    if self.current_input > 0 {
                        self.current_input -= 1;
                    } else {
                        self.current_input = self.inputs.len(); // Submit button
                    }
                }
                KeyCode::Down | KeyCode::Tab => {
                    if self.current_input < self.inputs.len() {
                        self.current_input += 1;
                    } else {
                        self.current_input = 0;
                    }
                }
                KeyCode::Enter => {
                    if self.current_input == self.inputs.len() {
                        return true; // Done
                    } else {
                        self.current_input += 1;
                    }
                }
                KeyCode::Esc => {
                    return true; // Also close
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
}

pub struct ActionsState {
    pub init_git: bool,
    pub bootstrap_flux: bool,
    pub list_state: ListState,
    pub focus: ActionsFocus,
    pub flux_modal: Option<ModalState>,
    pub git_modal: Option<ModalState>,
    pub action_trigger: Option<String>,
}

impl ActionsState {
    pub fn new(config: &crate::config::AppConfig) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            init_git: config.init_git_daemon,
            bootstrap_flux: config.bootstrap_flux,
            list_state,
            focus: ActionsFocus::List,
            flux_modal: Some(ModalState::new_flux(config)),
            git_modal: Some(ModalState::new_git(config)),
            action_trigger: None,
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> bool {
        if self.focus == ActionsFocus::ModalFlux {
            if let Some(modal) = &mut self.flux_modal {
                let closed = modal.handle_event(event);
                if closed {
                    self.focus = ActionsFocus::List;
                }
            }
            return false;
        }

        if self.focus == ActionsFocus::ModalGit {
            if let Some(modal) = &mut self.git_modal {
                let closed = modal.handle_event(event);
                if closed {
                    self.focus = ActionsFocus::List;
                }
            }
            return false;
        }

        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                    match self.focus {
                        ActionsFocus::List => {
                            if let Some(i) = self.list_state.selected() {
                                if i == 0 {
                                    self.list_state.select(Some(1));
                                } else {
                                    self.focus = ActionsFocus::Next; // Move to Next
                                    self.list_state.select(None);
                                }
                            }
                        }
                        ActionsFocus::Next => self.focus = ActionsFocus::Previous,
                        ActionsFocus::Previous => {
                            self.focus = ActionsFocus::List;
                            self.list_state.select(Some(0));
                        }
                        _ => {}
                    }
                }
                KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => match self.focus {
                    ActionsFocus::List => {
                        if let Some(i) = self.list_state.selected() {
                            if i == 1 {
                                self.list_state.select(Some(0));
                            } else {
                                self.focus = ActionsFocus::Previous;
                                self.list_state.select(None);
                            }
                        } else {
                            self.list_state.select(Some(1));
                        }
                    }
                    ActionsFocus::Previous => self.focus = ActionsFocus::Next,
                    ActionsFocus::Next => {
                        self.focus = ActionsFocus::List;
                        self.list_state.select(Some(1));
                    }
                    _ => {}
                },
                KeyCode::Left | KeyCode::Char('h') => {
                    if self.focus == ActionsFocus::Previous {
                        self.focus = ActionsFocus::Next;
                    } else if self.focus == ActionsFocus::Next {
                        self.focus = ActionsFocus::Previous;
                    }
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    if self.focus == ActionsFocus::Previous {
                        self.focus = ActionsFocus::Next;
                    } else if self.focus == ActionsFocus::Next {
                        self.focus = ActionsFocus::Previous;
                    }
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if self.focus == ActionsFocus::Previous {
                        self.action_trigger = Some("Previous".to_string());
                        return true;
                    } else if self.focus == ActionsFocus::Next {
                        self.action_trigger = Some("Next".to_string());
                        return true;
                    } else if self.focus == ActionsFocus::List
                        && let Some(i) = self.list_state.selected()
                    {
                        if i == 0 {
                            self.init_git = !self.init_git;
                            if self.init_git {
                                self.focus = ActionsFocus::ModalGit;
                            }
                        } else if i == 1 {
                            self.bootstrap_flux = !self.bootstrap_flux;
                            if self.bootstrap_flux {
                                self.focus = ActionsFocus::ModalFlux;
                            }
                        }
                    }
                }
                KeyCode::Char('e') => {
                    if self.focus == ActionsFocus::List
                        && let Some(i) = self.list_state.selected()
                    {
                        if i == 0 && self.init_git {
                            self.focus = ActionsFocus::ModalGit;
                        } else if i == 1 && self.bootstrap_flux {
                            self.focus = ActionsFocus::ModalFlux;
                        }
                    }
                }
                _ => {}
            }
        }
        false
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(area);

        let git_box = if self.init_git { "[x]" } else { "[ ]" };
        let flux_box = if self.bootstrap_flux { "[x]" } else { "[ ]" };

        let items = vec![
            ListItem::new(Span::raw(format!(
                "{} Initialize Git and Git Daemon{}",
                git_box,
                if self.init_git {
                    " (Press 'e' to configure)"
                } else {
                    ""
                }
            ))),
            ListItem::new(Span::raw(format!(
                "{} Bootstrap Flux{}",
                flux_box,
                if self.bootstrap_flux {
                    " (Press 'e' to configure)"
                } else {
                    ""
                }
            ))),
        ];

        let list_style = if self.focus == ActionsFocus::List {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let list = List::new(items)
            .highlight_style(list_style)
            .highlight_symbol("> ");

        f.render_stateful_widget(list, chunks[0], &mut self.list_state);

        let btn_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[1]);

        let prev_style = if self.focus == ActionsFocus::Previous {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        };
        let prev_btn = ratatui::widgets::Paragraph::new(Span::raw("   [ PREVIOUS ]   "))
            .style(prev_style)
            .alignment(ratatui::layout::Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(prev_style),
            );

        let next_style = if self.focus == ActionsFocus::Next {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        };
        let next_btn = ratatui::widgets::Paragraph::new(Span::raw("   [ NEXT ]   "))
            .style(next_style)
            .alignment(ratatui::layout::Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(next_style),
            );

        f.render_widget(next_btn, btn_chunks[0]);
        f.render_widget(prev_btn, btn_chunks[1]);

        let active_modal = if self.focus == ActionsFocus::ModalFlux {
            self.flux_modal.as_ref()
        } else if self.focus == ActionsFocus::ModalGit {
            self.git_modal.as_ref()
        } else {
            None
        };

        if let Some(modal) = active_modal {
            let popup_width = area.width.saturating_sub(8); // Almost full width of the screen!
            let popup_height = (modal.inputs.len() * 4 + 3 + 2) as u16; // 4 lines per input, 3 for button, 2 for borders
            let popup_area = ratatui::layout::Rect {
                x: area.x + (area.width.saturating_sub(popup_width)) / 2,
                y: area.y + (area.height.saturating_sub(popup_height)) / 2,
                width: popup_width,
                height: popup_height.min(area.height),
            };
            f.render_widget(Clear, popup_area);

            let popup_block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta))
                .title(modal.title);

            let inner_popup = popup_block.inner(popup_area);
            f.render_widget(popup_block, popup_area);

            let mut constraints = vec![];
            for _ in 0..modal.labels.len() {
                constraints.push(Constraint::Length(4)); // Give more vertical space!
            }
            constraints.push(Constraint::Length(3)); // Done button

            let input_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(constraints)
                .split(inner_popup);

            for (i, label) in modal.labels.iter().enumerate() {
                let style = if i == modal.current_input {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                let val = modal.inputs[i].value();

                let line = if i == modal.current_input {
                    let cursor = modal.inputs[i].cursor();
                    let mut before = String::new();
                    let mut cursor_char = String::from(" ");
                    let mut after = String::new();

                    for (idx, c) in val.chars().enumerate() {
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
                    ratatui::text::Line::from(Span::styled(val, style))
                };

                let p = Paragraph::new(line)
                    .wrap(ratatui::widgets::Wrap { trim: false })
                    .block(
                        Block::default()
                            .title(*label)
                            .borders(Borders::ALL)
                            .border_style(style),
                    );
                f.render_widget(p, input_chunks[i]);
            }

            let btn_style = if modal.current_input == modal.inputs.len() {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            };

            let done_btn = Paragraph::new(Span::raw("   [ DONE ]   "))
                .style(btn_style)
                .alignment(ratatui::layout::Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(btn_style),
                );

            f.render_widget(done_btn, input_chunks[modal.inputs.len()]);
        }
    }
}
