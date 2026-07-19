use crossterm::event::{Event, KeyCode};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph},
};

#[derive(PartialEq, Clone, Copy)]
pub enum SummaryFocus {
    Previous,
    Finish,
}

#[derive(PartialEq, Clone, Copy)]
pub enum ConfirmFocus {
    Yes,
    No,
}

pub struct SummaryState {
    pub focus: SummaryFocus,
    pub action_trigger: Option<String>,
    pub confirm_modal: bool,
    pub confirm_focus: ConfirmFocus,
}

impl SummaryState {
    pub fn new() -> Self {
        Self {
            focus: SummaryFocus::Finish,
            action_trigger: None,
            confirm_modal: false,
            confirm_focus: ConfirmFocus::No,
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> bool {
        if let Event::Key(key) = event {
            if self.confirm_modal {
                match key.code {
                    KeyCode::Left | KeyCode::Char('h') => {
                        self.confirm_focus = ConfirmFocus::Yes;
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        self.confirm_focus = ConfirmFocus::No;
                    }
                    KeyCode::Tab | KeyCode::BackTab => {
                        self.confirm_focus = if self.confirm_focus == ConfirmFocus::Yes {
                            ConfirmFocus::No
                        } else {
                            ConfirmFocus::Yes
                        };
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        if self.confirm_focus == ConfirmFocus::Yes {
                            self.action_trigger = Some("Finish".to_string());
                            return true;
                        } else {
                            self.confirm_modal = false;
                        }
                    }
                    KeyCode::Esc => {
                        self.confirm_modal = false;
                    }
                    _ => {}
                }
                return false;
            }

            match key.code {
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Up => {
                    self.focus = SummaryFocus::Previous;
                }
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Down => {
                    self.focus = SummaryFocus::Finish;
                }
                KeyCode::Tab | KeyCode::BackTab => {
                    self.focus = if self.focus == SummaryFocus::Previous {
                        SummaryFocus::Finish
                    } else {
                        SummaryFocus::Previous
                    };
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if self.focus == SummaryFocus::Previous {
                        self.action_trigger = Some("Previous".to_string());
                        return true;
                    } else if self.focus == SummaryFocus::Finish {
                        self.confirm_modal = true;
                        self.confirm_focus = ConfirmFocus::No; // Default to safe option
                    }
                }
                _ => {}
            }
        }
        false
    }

    pub fn render(
        &mut self,
        f: &mut Frame,
        area: Rect,
        config: &crate::config::AppConfig,
        checked_paths: &std::collections::HashSet<String>,
        actions: &crate::actions::ActionsState,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(area);

        let mut lines = Vec::new();

        lines.push(ratatui::text::Line::from(Span::styled(
            "GitOps Structure Generation",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(ratatui::text::Line::from(format!(
            "  Target Directory: {}",
            config.gitops_dir_path
        )));
        lines.push(ratatui::text::Line::from(format!(
            "  Cluster Name: {}",
            config.new_cluster_name
        )));

        lines.push(ratatui::text::Line::from(""));
        lines.push(ratatui::text::Line::from(Span::styled(
            "Selected Components:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));

        if checked_paths.is_empty() {
            lines.push(ratatui::text::Line::from("  (None selected)"));
        } else {
            // Group paths by top-level directory
            let mut categorized: std::collections::BTreeMap<String, Vec<String>> =
                std::collections::BTreeMap::new();
            for path in checked_paths {
                let parts: Vec<&str> = path.split('/').collect();
                if parts.len() > 1 {
                    categorized
                        .entry(parts[0].to_string())
                        .or_default()
                        .push(parts[1..].join("/"));
                } else {
                    categorized
                        .entry("root".to_string())
                        .or_default()
                        .push(path.clone());
                }
            }

            for (category, items) in categorized {
                lines.push(ratatui::text::Line::from(Span::styled(
                    format!("  - {}", category),
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                )));
                let mut sorted_items = items.clone();
                sorted_items.sort();
                for item in sorted_items {
                    lines.push(ratatui::text::Line::from(format!("      - {}", item)));
                }
            }
        }

        lines.push(ratatui::text::Line::from(""));

        if actions.init_git || actions.bootstrap_flux {
            lines.push(ratatui::text::Line::from(Span::styled(
                "Post-Generation Actions",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )));
            if actions.init_git {
                lines.push(ratatui::text::Line::from(
                    "  [x] Initialize Local Git Repository & Push to Remote",
                ));
                lines.push(ratatui::text::Line::from(Span::styled(
                    "      DANGER: Will FORCE PUSH and OVERWRITE remote history!",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                )));
            }
            if actions.bootstrap_flux {
                lines.push(ratatui::text::Line::from("  [x] Bootstrap Flux CD"));
                if let Some(modal) = &actions.flux_modal {
                    lines.push(ratatui::text::Line::from(format!(
                        "      - Git URL: {}",
                        modal.inputs[0].value()
                    )));
                    lines.push(ratatui::text::Line::from(format!(
                        "      - Branch: {}",
                        modal.inputs[1].value()
                    )));
                    lines.push(ratatui::text::Line::from(format!(
                        "      - Path: {}",
                        modal.inputs[2].value()
                    )));
                    lines.push(ratatui::text::Line::from(format!(
                        "      - Kubeconfig: {}",
                        modal.inputs[3].value()
                    )));
                }
            }
        } else {
            lines.push(ratatui::text::Line::from(Span::styled(
                "No Post-Generation Actions Selected",
                Style::default().fg(Color::DarkGray),
            )));
        }

        let p = Paragraph::new(lines).block(Block::default());
        f.render_widget(p, chunks[0]);

        let btn_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[1]);

        let prev_style = if self.focus == SummaryFocus::Previous {
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

        let finish_style = if self.focus == SummaryFocus::Finish {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        };
        let finish_btn = ratatui::widgets::Paragraph::new(Span::raw("   [ FINISH & EXECUTE ]   "))
            .style(finish_style)
            .alignment(ratatui::layout::Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(finish_style),
            );

        f.render_widget(finish_btn, btn_chunks[0]);
        f.render_widget(prev_btn, btn_chunks[1]);

        if self.confirm_modal {
            let popup_width = 40;
            let popup_area = ratatui::layout::Rect {
                x: area.x + (area.width.saturating_sub(popup_width)) / 2,
                y: area.y + area.height / 2 - 3,
                width: popup_width,
                height: 7,
            };

            f.render_widget(ratatui::widgets::Clear, popup_area);

            let popup_block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .title(" CONFIRM EXECUTION ");

            let inner_popup = popup_block.inner(popup_area);
            f.render_widget(popup_block, popup_area);

            let msg = Paragraph::new(Span::raw("Are you sure you want to proceed?"))
                .alignment(ratatui::layout::Alignment::Center);

            let modal_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(2), Constraint::Length(3)])
                .split(inner_popup);

            f.render_widget(msg, modal_chunks[0]);

            let modal_btn_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(modal_chunks[1]);

            let yes_style = if self.confirm_focus == ConfirmFocus::Yes {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Green)
            };

            let no_style = if self.confirm_focus == ConfirmFocus::No {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Red)
            };

            let yes_btn = Paragraph::new(Span::raw(" [ YES ] "))
                .style(yes_style)
                .alignment(ratatui::layout::Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(yes_style),
                );

            let no_btn = Paragraph::new(Span::raw(" [ NO ] "))
                .style(no_style)
                .alignment(ratatui::layout::Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(no_style),
                );

            f.render_widget(yes_btn, modal_btn_chunks[0]);
            f.render_widget(no_btn, modal_btn_chunks[1]);
        }
    }
}
