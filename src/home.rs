use crossterm::event::{Event, KeyCode};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph},
};

#[derive(PartialEq, Clone, Copy)]
pub enum HomeOption {
    Start,
    EditConfig,
    Quit,
}

pub struct HomeState {
    pub selected: HomeOption,
    pub action_trigger: Option<HomeOption>,
}

impl HomeState {
    pub fn new() -> Self {
        Self {
            selected: HomeOption::Start,
            action_trigger: None,
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> bool {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.selected = match self.selected {
                        HomeOption::Start => HomeOption::Quit,
                        HomeOption::EditConfig => HomeOption::Start,
                        HomeOption::Quit => HomeOption::EditConfig,
                    };
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.selected = match self.selected {
                        HomeOption::Start => HomeOption::EditConfig,
                        HomeOption::EditConfig => HomeOption::Quit,
                        HomeOption::Quit => HomeOption::Start,
                    };
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    self.action_trigger = Some(self.selected);
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let ascii_art = r#"
   ____ _ _    ___             ____              _       _                   _____ _   _ ___ 
  / ___(_) |_ / _ \ _ __  ___ | __ )  ___   ___ | |_ ___| |_ _ __ __ _ _ __ |_   _| | | |_ _|
 | |  _| | __| | | | '_ \/ __||  _ \ / _ \ / _ \| __/ __| __| '__/ _` | '_ \  | | | | | || | 
 | |_| | | |_| |_| | |_) \__ \| |_) | (_) | (_) | |_\__ \ |_| | | (_| | |_) | | | | |_| || | 
  \____|_|\__|\___/| .__/|___/|____/ \___/ \___/ \__|___/\__|_|  \__,_| .__/  |_|  \___/|___|
                   |_|                                                |_|                    
"#;

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(20), // Top spacing
                Constraint::Length(8),      // ASCII Art
                Constraint::Length(2),      // Space
                Constraint::Length(3),      // Start Button
                Constraint::Length(3),      // Edit Config Button
                Constraint::Length(3),      // Quit Button
                Constraint::Min(0),         // Bottom spacing
            ])
            .split(area);

        let art_paragraph = Paragraph::new(ascii_art)
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center);

        f.render_widget(art_paragraph, chunks[1]);

        let start_style = if self.selected == HomeOption::Start {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Green)
        };

        let start_btn = Paragraph::new(Span::raw(" [ START BOOTSTRAPPING ] "))
            .style(start_style)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(start_style),
            );

        let edit_style = if self.selected == HomeOption::EditConfig {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Yellow)
        };

        let edit_btn = Paragraph::new(Span::raw(" [ EDIT CONFIG ] "))
            .style(edit_style)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(edit_style),
            );

        let quit_style = if self.selected == HomeOption::Quit {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Red)
        };

        let quit_btn = Paragraph::new(Span::raw(" [ QUIT ] "))
            .style(quit_style)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(quit_style),
            );

        // Center the buttons horizontally
        let btn_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(60),
                Constraint::Percentage(20),
            ]);

        let start_chunks = btn_layout.split(chunks[3]);
        let edit_chunks = btn_layout.split(chunks[4]);
        let quit_chunks = btn_layout.split(chunks[5]);

        f.render_widget(start_btn, start_chunks[1]);
        f.render_widget(edit_btn, edit_chunks[1]);
        f.render_widget(quit_btn, quit_chunks[1]);
    }
}
