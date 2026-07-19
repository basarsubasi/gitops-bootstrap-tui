use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, List, ListItem},
};
use std::{io, time::Duration};

use crate::config::AppConfig;
use crate::explorer::{ExplorerFocus, ExplorerState};
use crate::wizard::WizardState;

pub enum View {
    Home(crate::home::HomeState),
    Wizard(WizardState),
    Loading,
    Explorer(ExplorerState),
    Actions(crate::actions::ActionsState),
    Summary(crate::summary::SummaryState),
    Executing(crate::executing::ExecutingState),
}

pub struct App {
    pub should_quit: bool,
    pub view: View,
    pub config: AppConfig,
    pub pending_generation: Option<(
        std::collections::HashSet<String>,
        std::collections::HashMap<String, String>,
    )>,
    pub pending_actions: Option<crate::actions::ActionsState>,
    pub saved_explorer_state: Option<ExplorerState>,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        Self {
            should_quit: false,
            view: View::Home(crate::home::HomeState::new()),
            config,
            pending_generation: None,
            pending_actions: None,
            saved_explorer_state: None,
        }
    }
}

pub fn run_app(config: AppConfig) -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        orig_hook(panic_info);
    }));

    let mut app = App::new(config);
    let res = run_app_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

fn run_app_loop<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()>
where
    std::io::Error: From<<B as Backend>::Error>,
{
    loop {
        if let View::Executing(exec) = &mut app.view {
            exec.poll_events();
        }

        terminal.draw(|f| ui(f, app))?;

        if event::poll(Duration::from_millis(250))? {
            let ev = event::read()?;

            if let Event::Key(key) = ev {
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    app.should_quit = true;
                    return Ok(());
                }

                match &mut app.view {
                    View::Executing(exec) => {
                        let triggered = exec.handle_event(&ev);
                        if triggered {
                            if exec.should_quit {
                                app.should_quit = true;
                            } else if exec.go_back {
                                if let Some(actions) = app.pending_actions.take() {
                                    app.view = View::Actions(actions);
                                } else {
                                    app.should_quit = true;
                                }
                            }
                        }
                    }
                    View::Home(home) => {
                        let triggered = home.handle_event(&ev);
                        if triggered && let Some(action) = home.action_trigger {
                            match action {
                                crate::home::HomeOption::Start => {
                                    app.view = View::Wizard(WizardState::new(&app.config));
                                }
                                crate::home::HomeOption::EditConfig => {
                                    let home_dir = directories::UserDirs::new()
                                        .unwrap()
                                        .home_dir()
                                        .to_path_buf();
                                    let config_file = home_dir
                                        .join(".config")
                                        .join("gitops-bootstrap-tui")
                                        .join("config.json");

                                    let _ = disable_raw_mode();
                                    let _ = execute!(
                                        io::stdout(),
                                        LeaveAlternateScreen,
                                       
                                    );

                                    let editor = std::env::var("EDITOR")
                                        .unwrap_or_else(|_| "vim".to_string());
                                    let _ = std::process::Command::new(editor)
                                        .arg(&config_file)
                                        .status();

                                    let _ = enable_raw_mode();
                                    let _ = execute!(
                                        io::stdout(),
                                        EnterAlternateScreen
                                    );
                                    let _ = terminal.clear();

                                    if let Ok(new_config) = crate::config::AppConfig::load() {
                                        app.config = new_config;
                                    }
                                    home.action_trigger = None;
                                }
                                crate::home::HomeOption::Quit => {
                                    app.should_quit = true;
                                }
                            }
                        }
                    }
                    View::Wizard(wizard) => {
                        let should_quit = wizard.handle_event(&ev);
                        if should_quit {
                            app.should_quit = true;
                        } else if wizard.action == crate::wizard::WizardAction::Previous {
                            app.view = View::Home(crate::home::HomeState::new());
                        } else if wizard.action == crate::wizard::WizardAction::Next {
                            wizard.action = crate::wizard::WizardAction::None; // Reset
                            let template_repo_url = wizard.inputs[0].value().to_string();
                            let base_dir_path = wizard.inputs[1].value().to_string();
                            app.config.template_repo_url = template_repo_url.clone();
                            app.config.base_dir_path = base_dir_path.clone();
                            app.config.new_cluster_name = wizard.inputs[2].value().to_string();
                            app.config.gitops_dir_path = wizard.inputs[3].value().to_string();

                            let expanded_gitops_path = if let Some(stripped) =
                                app.config.gitops_dir_path.strip_prefix("~/")
                            {
                                if let Some(home) =
                                    directories::UserDirs::new().map(|d| d.home_dir().to_path_buf())
                                {
                                    home.join(stripped)
                                } else {
                                    std::path::PathBuf::from(&app.config.gitops_dir_path)
                                }
                            } else {
                                std::path::PathBuf::from(&app.config.gitops_dir_path)
                            };
                            let target_cluster =
                                expanded_gitops_path.join(&app.config.new_cluster_name);
                            if target_cluster.exists() {
                                wizard.error_message = Some(format!(
                                    "Directory already exists:\n{}",
                                    target_cluster.display()
                                ));
                                continue;
                            }

                            let _ = app.config.save();

                            app.view = View::Loading;
                            terminal.draw(|f| ui(f, app))?;

                            if let Ok(git_mgr) = crate::git::GitManager::new(&template_repo_url) {
                                if git_mgr.sync().is_ok() {
                                    let base_path = git_mgr.repo_dir.join(&base_dir_path);
                                    app.view = View::Explorer(ExplorerState::new(base_path));
                                } else {
                                    app.should_quit = true;
                                }
                            } else {
                                app.should_quit = true;
                            }
                        }
                    }
                    View::Explorer(state) => {
                        if state.preview_content.is_some() {
                            state.preview_content = None;
                            continue;
                        }

                        if state.error_message.is_some() {
                            state.error_message = None;
                            continue;
                        }

                        match key.code {
                            KeyCode::Char('q') => app.should_quit = true,
                            KeyCode::Down | KeyCode::Char('j') => state.handle_down(),
                            KeyCode::Up | KeyCode::Char('k') => state.handle_up(),
                            KeyCode::Left | KeyCode::Char('h') => state.handle_left(),
                            KeyCode::Right | KeyCode::Char('l') => state.handle_right(),
                            KeyCode::Tab => state.handle_tab(),
                            KeyCode::BackTab => state.handle_backtab(),
                            KeyCode::Enter | KeyCode::Char(' ') => {
                                if state.focus == ExplorerFocus::Previous {
                                    // Go back to Wizard
                                    app.view = View::Wizard(WizardState::new(&app.config));
                                } else if state.focus == ExplorerFocus::Next {
                                    // Save state and transition to Actions
                                    app.pending_generation = Some((
                                        state.checked_paths.clone(),
                                        state.customized_paths.clone(),
                                    ));

                                    // We need to extract state, so we take it out temporarily
                                    // but we can't consume it easily from &mut app.view.
                                    // We will use std::mem::replace to extract it.
                                    let mut temp_view = View::Loading;
                                    std::mem::swap(&mut app.view, &mut temp_view);
                                    if let View::Explorer(owned_state) = temp_view {
                                        app.saved_explorer_state = Some(owned_state);
                                    }

                                    app.view = View::Actions(crate::actions::ActionsState::new(
                                        &app.config,
                                    ));
                                } else {
                                    if let Some(idx) = state.list_state.selected()
                                        && let Some(item) = state.flat_list.get(idx).cloned()
                                    {
                                        if item.is_leaf {
                                            state.toggle_current();
                                        } else {
                                            state.toggle_expand();
                                        }
                                    }
                                }
                            }
                            KeyCode::Char('p') => {
                                if state.focus == ExplorerFocus::Tree
                                    && let Some(idx) = state.list_state.selected()
                                    && let Some(item) = state.flat_list.get(idx)
                                    && let Some(yaml) = state.customized_paths.get(&item.path)
                                {
                                    state.preview_content = Some(yaml.clone());
                                }
                            }
                            KeyCode::Char('e') => {
                                if state.focus == ExplorerFocus::Tree
                                    && let Some(idx) = state.list_state.selected()
                                    && let Some(item) = state.flat_list.get(idx).cloned()
                                {
                                    if item.is_leaf && item.is_helm {
                                        let full_path = state.root_path.join(&item.path);
                                        let initial_values =
                                            match state.customized_paths.get(&item.path) {
                                                Some(v) => v.clone(),
                                                None => crate::helm::fetch_helm_values(&full_path)
                                                    .unwrap_or_default(),
                                            };

                                        let _ = disable_raw_mode();
                                        let _ = execute!(
                                            io::stdout(),
                                            LeaveAlternateScreen,
                                           
                                        );

                                        if let Ok(Some(edited)) =
                                            crate::helm::edit_yaml(&initial_values)
                                        {
                                            state
                                                .customized_paths
                                                .insert(item.path.clone(), edited);
                                        }

                                        let _ = enable_raw_mode();
                                        let _ = execute!(
                                            io::stdout(),
                                            EnterAlternateScreen
                                        );
                                        let _ = terminal.clear();
                                    } else {
                                        state.error_message = Some(
                                            "Component is not editable (Not a Helm Release)"
                                                .to_string(),
                                        );
                                    }
                                }
                            }
                            KeyCode::Char('u') => {
                                state.undo_current();
                            }
                            KeyCode::Char('c') => {
                                let home_dir = directories::UserDirs::new()
                                    .unwrap()
                                    .home_dir()
                                    .to_path_buf();
                                let config_file = home_dir
                                    .join(".config")
                                    .join("gitops-bootstrap-tui")
                                    .join("config.json");

                                let _ = disable_raw_mode();
                                let _ = execute!(
                                    io::stdout(),
                                    LeaveAlternateScreen,
                                   
                                );

                                let editor =
                                    std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
                                let _ = std::process::Command::new(editor)
                                    .arg(&config_file)
                                    .status();

                                // Reload config!
                                if let Ok(new_cfg) = crate::config::AppConfig::load() {
                                    app.config = new_cfg;
                                }

                                let _ = enable_raw_mode();
                                let _ = execute!(
                                    io::stdout(),
                                    EnterAlternateScreen
                                );
                                let _ = terminal.clear();
                            }
                            _ => {}
                        }
                    }
                    View::Actions(actions) => {
                        let triggered = actions.handle_event(&ev);
                        if triggered && let Some(trigger) = &actions.action_trigger {
                            if trigger == "Next" {
                                // Save the preferences back to AppConfig!
                                if let View::Actions(ref owned_actions) = app.view {
                                    app.config.init_git = owned_actions.init_git;
                                    app.config.bootstrap_flux = owned_actions.bootstrap_flux;
                                    if let Some(modal) = &owned_actions.flux_modal {
                                        app.config.flux_git_url =
                                            modal.inputs[0].value().to_string();
                                        app.config.flux_git_branch =
                                            modal.inputs[1].value().to_string();
                                        app.config.flux_kubeconfig =
                                            modal.inputs[3].value().to_string();
                                        app.config.flux_ssh_key_path =
                                            modal.inputs[4].value().to_string();
                                    }
                                    let _ = app.config.save();
                                }

                                // We need to move the state
                                let mut temp_view = View::Loading;
                                std::mem::swap(&mut app.view, &mut temp_view);
                                if let View::Actions(owned_actions) = temp_view {
                                    app.pending_actions = Some(owned_actions);
                                }
                                app.view = View::Summary(crate::summary::SummaryState::new());
                            } else if trigger == "Previous" {
                                // Restore ExplorerState
                                if let Some(saved_state) = app.saved_explorer_state.take() {
                                    app.view = View::Explorer(saved_state);
                                } else {
                                    app.should_quit = true;
                                }
                            }
                        }
                    }
                    View::Summary(summary) => {
                        let triggered = summary.handle_event(&ev);
                        if triggered && let Some(trigger) = &summary.action_trigger {
                            if trigger == "Finish" {
                                crate::ui::start_execution_thread(app);
                            } else if trigger == "Previous"
                                && let Some(saved_actions) = app.pending_actions.take()
                            {
                                app.view = View::Actions(saved_actions);
                            }
                        }
                    }
                    View::Loading => {}
                }
            }
        }
        if app.should_quit {
            return Ok(());
        }
    }
}

pub fn ui(f: &mut Frame, app: &mut App) {
    let size = f.area();

    let mut title_spans = vec![];
    
    match &app.view {
        View::Executing(_) => {
            title_spans.push(Span::styled(" EXECUTING ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        }
        View::Home(_) => {
            title_spans.push(Span::styled(" HOME ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        }
        _ => {
            title_spans.push(Span::raw(" "));
            
            let active = matches!(app.view, View::Wizard(_) | View::Loading);
            title_spans.push(Span::styled("WIZARD", if active { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::DarkGray) }));
            
            title_spans.push(Span::styled(" > ", Style::default().fg(Color::DarkGray)));
            
            let active = matches!(app.view, View::Explorer(_));
            title_spans.push(Span::styled("COMPONENTS", if active { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::DarkGray) }));
            
            title_spans.push(Span::styled(" > ", Style::default().fg(Color::DarkGray)));
            
            let active = matches!(app.view, View::Actions(_));
            title_spans.push(Span::styled("ACTIONS", if active { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::DarkGray) }));
            
            title_spans.push(Span::styled(" > ", Style::default().fg(Color::DarkGray)));
            
            let active = matches!(app.view, View::Summary(_));
            title_spans.push(Span::styled("SUMMARY", if active { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::DarkGray) }));
            
            title_spans.push(Span::raw(" "));
        }
    }

    let legend = match &app.view {
        View::Executing(_) => "",
        View::Home(_) => " [Tab] Navigate   [Enter] Select ",
        View::Wizard(_) => " [Tab/Shift+Tab] Navigate   [Enter] Input   [Arrows] Buttons ",
        View::Loading => "",
        View::Summary(_) => " [Tab] Focus   [Arrows] Navigate   [Enter] Submit ",
        View::Actions(state) => {
            if state.focus == crate::actions::ActionsFocus::ModalFlux {
                " [Tab/Shift+Tab] Navigate   [Enter] Input   [Esc] Close "
            } else {
                " [Tab] Focus   [Arrows] Navigate   [Enter] Toggle/Submit   [e] Configure "
            }
        }
        View::Explorer(state) => {
            if state.error_message.is_some() {
                " [Any Key] Close Error "
            } else if state.preview_content.is_some() {
                " [Any Key] Close Preview "
            } else if state.focus == ExplorerFocus::Tree {
                " [Tab] Focus   [Arrows] Navigate   [Enter/Space] Toggle/Expand   [e] Edit   [u] Undo Changes:   [p] Preview   [c] Edit Config "
            } else {
                " [Tab] Focus   [Arrows] Navigate   [Enter] Submit "
            }
        }
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(ratatui::text::Line::from(title_spans))
        .title_bottom(
            ratatui::text::Line::from(Span::styled(
                legend, Style::default().fg(Color::DarkGray)))
                .alignment(ratatui::layout::Alignment::Right),
        );

    let inner_area = block.inner(size);
    f.render_widget(block, size);

    match &mut app.view {
        View::Executing(executing) => executing.render(f, inner_area),
        View::Home(home) => {
            home.render(f, inner_area);
        }
        View::Wizard(wizard) => {
            wizard.render(f, inner_area);
        }
        View::Actions(actions) => {
            actions.render(f, inner_area);
        }
        View::Summary(summary) => {
            let default_set = std::collections::HashSet::new();
            let checked_paths = if let Some(generation) = &app.pending_generation {
                &generation.0
            } else {
                &default_set
            };
            // Since app.pending_actions was moved out, we borrow it from app
            if let Some(actions) = &app.pending_actions {
                summary.render(f, inner_area, &app.config, checked_paths, actions);
            }
        }
        View::Loading => {
            let p =
                ratatui::widgets::Paragraph::new("Cloning/Pulling Git Repository... Please wait.")
                    .style(
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )
                    .alignment(ratatui::layout::Alignment::Center);
            f.render_widget(p, inner_area);
        }
        View::Explorer(state) => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(3)])
                .split(inner_area);

            let mut list_items = vec![];

            for item in &state.flat_list {
                let indent = "  ".repeat(item.depth);
                let is_checked = state.checked_paths.contains(&item.path);

                let prefix = if item.is_leaf {
                    if is_checked { "[x] " } else { "[ ] " }
                } else {
                    if state.expanded_paths.contains(&item.path) {
                        "[-] "
                    } else {
                        "[+] "
                    }
                };

                let suffix = if state.customized_paths.contains_key(&item.path) {
                    " (customized)"
                } else {
                    ""
                };

                let color = if is_checked {
                    Color::Green
                } else if item.is_leaf {
                    Color::White
                } else {
                    Color::Blue
                };

                list_items.push(ListItem::new(Span::styled(
                    format!("{}{}{}{}", indent, prefix, item.name, suffix),
                    Style::default().fg(color),
                )));
            }

            let tree_style = if state.focus == ExplorerFocus::Tree {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let tree_list = List::new(list_items)
                .highlight_style(tree_style)
                .highlight_symbol("> ");

            f.render_stateful_widget(tree_list, chunks[0], &mut state.list_state);

            let btn_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(chunks[1]);

            let prev_style = if state.focus == ExplorerFocus::Previous {
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

            let next_style = if state.focus == ExplorerFocus::Next {
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

            // Swap: Next on left (0), Previous on right (1)
            // Wait, standard UI is Previous (left) Next (right). The user requested to move previous to right:
            // "move the previous button to right"
            f.render_widget(next_btn, btn_chunks[0]);
            f.render_widget(prev_btn, btn_chunks[1]);

            if let Some(preview) = &state.preview_content {
                let popup_area = ratatui::layout::Rect {
                    x: inner_area.x + 4,
                    y: inner_area.y + 2,
                    width: inner_area.width.saturating_sub(8),
                    height: inner_area.height.saturating_sub(4),
                };
                f.render_widget(ratatui::widgets::Clear, popup_area);

                let popup_block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Magenta))
                    .title(ratatui::text::Line::from(Span::styled(
                        " PREVIEW PATCH ",
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD),
                    )));

                let text = ratatui::widgets::Paragraph::new(preview.as_str())
                    .style(Style::default().fg(Color::White))
                    .block(popup_block)
                    .wrap(ratatui::widgets::Wrap { trim: true });

                f.render_widget(text, popup_area);
            } else if let Some(error) = &state.error_message {
                let popup_area = ratatui::layout::Rect {
                    x: inner_area.x + 4,
                    y: inner_area.y + inner_area.height / 2 - 2,
                    width: inner_area.width.saturating_sub(8),
                    height: 4,
                };
                f.render_widget(ratatui::widgets::Clear, popup_area);

                let popup_block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Red))
                    .title(ratatui::text::Line::from(Span::styled(
                        " ERROR ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    )));

                let text = ratatui::widgets::Paragraph::new(error.as_str())
                    .style(Style::default().fg(Color::White))
                    .alignment(ratatui::layout::Alignment::Center)
                    .block(popup_block);

                f.render_widget(text, popup_area);
            }
        }
    }
}


pub fn start_execution_thread(app: &mut App) {
    let (tx, rx) = std::sync::mpsc::channel();
    let (input_tx, input_rx) = std::sync::mpsc::channel();
    
    let config = app.config.clone();
    let pending_generation = app.pending_generation.clone();

    let init_git = app.pending_actions.as_ref().map(|a| a.init_git).unwrap_or(false);
    let bootstrap_flux = app.pending_actions.as_ref().map(|a| a.bootstrap_flux).unwrap_or(false);
    
    let (git_url_str, initial_branch, path_str, kubeconfig_str, ssh_key_str) = if let Some(a) = &app.pending_actions {
        if let Some(m) = &a.flux_modal {
            (
                m.inputs[0].value().to_string(),
                m.inputs[1].value().to_string(),
                m.inputs[2].value().to_string(),
                m.inputs[3].value().to_string(),
                m.inputs[4].value().to_string()
            )
        } else {
            ("".to_string(), "main".to_string(), "".to_string(), "".to_string(), "".to_string())
        }
    } else {
        ("".to_string(), "main".to_string(), "".to_string(), "".to_string(), "".to_string())
    };

    std::thread::spawn(move || {
        let git_url = &git_url_str;
        let branch = &initial_branch;
        let initial_branch = &initial_branch;
        let path = &path_str;
        let kubeconfig = &kubeconfig_str;
        let ssh_key = &ssh_key_str;

        fn run_interactive_cmd(
            mut cmd: std::process::Command,
            name: &str,
            log_tx: &std::sync::mpsc::Sender<crate::executing::ExecutionEvent>,
            input_rx: &std::sync::mpsc::Receiver<String>,
        ) -> Result<(), String> {
            use std::io::{Read, Write};
            cmd.stdin(std::process::Stdio::piped())
               .stdout(std::process::Stdio::piped())
               .stderr(std::process::Stdio::piped());

            let mut child = cmd.spawn().map_err(|e| e.to_string())?;

            let mut stdout = child.stdout.take().unwrap();
            let mut stderr = child.stderr.take().unwrap();
            let mut stdin = child.stdin.take().unwrap();

            let tx_out = log_tx.clone();
            std::thread::spawn(move || {
                let mut buf = [0; 256];
                while let Ok(n) = stdout.read(&mut buf) {
                    if n == 0 { break; }
                    let s = String::from_utf8_lossy(&buf[..n]).to_string();
                    let _ = tx_out.send(crate::executing::ExecutionEvent::LogChunk(s));
                }
            });

            let tx_err = log_tx.clone();
            std::thread::spawn(move || {
                let mut buf = [0; 256];
                while let Ok(n) = stderr.read(&mut buf) {
                    if n == 0 { break; }
                    let s = String::from_utf8_lossy(&buf[..n]).to_string();
                    let _ = tx_err.send(crate::executing::ExecutionEvent::LogChunk(s));
                }
            });

            loop {
                if let Ok(Some(status)) = child.try_wait() {
                    if status.success() {
                        return Ok(());
                    } else {
                        return Err(format!("{} failed with exit code {}", name, status));
                    }
                }

                while let Ok(input) = input_rx.try_recv() {
                    let _ = stdin.write_all(input.as_bytes());
                    let _ = stdin.flush();
                }

                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }

    if let Some((checked_paths, customized_paths)) = pending_generation {
        let _ = tx.send(crate::executing::ExecutionEvent::Log("[1/3] Generating Output GitOps Directory...".to_string()));
        if let Ok(git_mgr) = crate::git::GitManager::new(&config.template_repo_url) {
            let expanded_gitops_path = if config.gitops_dir_path.starts_with("~/") {
                if let Some(home) = directories::UserDirs::new().map(|d| d.home_dir().to_path_buf())
                {
                    home.join(&config.gitops_dir_path[2..])
                        .to_string_lossy()
                        .to_string()
                } else {
                    config.gitops_dir_path.clone()
                }
            } else {
                config.gitops_dir_path.clone()
            };

            if let Err(e) = crate::generate::finalize_generation(
                &git_mgr.repo_dir,
                &expanded_gitops_path,
                &config.base_dir_path,
                &config.new_cluster_name,
                &checked_paths,
                &customized_paths,
            ) {
                let _ = tx.send(crate::executing::ExecutionEvent::Log(format!("ERROR generating: {:?}", e)));
            } else {
                let _ = tx.send(crate::executing::ExecutionEvent::Log(format!(
                    "✓ Successfully generated GitOps directory at {}",
                    expanded_gitops_path
                )));
                if true {
                    if init_git {
                        let _ = tx.send(crate::executing::ExecutionEvent::Log("[2/3] Initializing Git Repository & Pushing to Remote...".to_string()));
                        let target_dir = std::path::Path::new(&expanded_gitops_path);


                        if ssh_key.is_empty() {
                            let _ = tx.send(crate::executing::ExecutionEvent::Log("ERROR: Git SSH Key Path is required for pushing to remote and bootstrapping.".to_string()));
                            { let _ = tx.send(crate::executing::ExecutionEvent::Error("Operation failed. Press ESC to go back.".into())); return; }
                        }

                        let init_output = std::process::Command::new("git")
                            .arg("init")
                            .arg(format!("--initial-branch={}", initial_branch))
                            .current_dir(target_dir)
                            .output();
                        match init_output {
                            Ok(out) if !out.status.success() => {
                                let stderr = String::from_utf8_lossy(&out.stderr);
                                let _ = tx.send(crate::executing::ExecutionEvent::Log(format!("ERROR: Failed to initialize git repository:\n{}", stderr.trim())));
                                { let _ = tx.send(crate::executing::ExecutionEvent::Error("Operation failed. Press ESC to go back.".into())); return; }
                            }
                            Err(e) => {
                                let _ = tx.send(crate::executing::ExecutionEvent::Log(format!("ERROR: Failed to execute git init: {}", e)));
                                { let _ = tx.send(crate::executing::ExecutionEvent::Error("Operation failed. Press ESC to go back.".into())); return; }
                            }
                            _ => {}
                        }

                        let add_output = std::process::Command::new("git")
                            .arg("add")
                            .arg(".")
                            .current_dir(target_dir)
                            .output();
                        match add_output {
                            Ok(out) if !out.status.success() => {
                                let stderr = String::from_utf8_lossy(&out.stderr);
                                let _ = tx.send(crate::executing::ExecutionEvent::Log(format!("ERROR: Failed to add files to git repository:\n{}", stderr.trim())));
                                { let _ = tx.send(crate::executing::ExecutionEvent::Error("Operation failed. Press ESC to go back.".into())); return; }
                            }
                            Err(e) => {
                                let _ = tx.send(crate::executing::ExecutionEvent::Log(format!("ERROR: Failed to execute git add: {}", e)));
                                { let _ = tx.send(crate::executing::ExecutionEvent::Error("Operation failed. Press ESC to go back.".into())); return; }
                            }
                            _ => {}
                        }

                        let commit_output = std::process::Command::new("git")
                            .arg("commit")
                            .arg("-m")
                            .arg("Initial GitOps Commit")
                            .current_dir(target_dir)
                            .output();
                        match commit_output {
                            Ok(out) if !out.status.success() => {
                                let stderr = String::from_utf8_lossy(&out.stderr);
                                let stdout = String::from_utf8_lossy(&out.stdout);
                                if stdout.contains("nothing to commit") || stderr.contains("nothing to commit") {
                                    let _ = tx.send(crate::executing::ExecutionEvent::Log("ℹ Nothing to commit (working tree clean).".to_string()));
                                } else {
                                    let _ = tx.send(crate::executing::ExecutionEvent::Log(format!("ERROR: Failed to commit to git repository:\n{}", stderr.trim())));
                                    { let _ = tx.send(crate::executing::ExecutionEvent::Error("Operation failed. Press ESC to go back.".into())); return; }
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(crate::executing::ExecutionEvent::Log(format!("ERROR: Failed to execute git commit: {}", e)));
                                { let _ = tx.send(crate::executing::ExecutionEvent::Error("Operation failed. Press ESC to go back.".into())); return; }
                            }
                            _ => {}
                        }

                        let remote_add_output = std::process::Command::new("git")
                            .arg("remote")
                            .arg("add")
                            .arg("origin")
                            .arg(git_url)
                            .current_dir(target_dir)
                            .output();
                        match remote_add_output {
                            Ok(out) if !out.status.success() => {
                                let stderr = String::from_utf8_lossy(&out.stderr);
                                if stderr.contains("already exists") {
                                    let _ = tx.send(crate::executing::ExecutionEvent::Log("\x1b[1;33mℹ Remote origin already exists, skipping addition.\x1b[0m".to_string()));
                                } else {
                                    let _ = tx.send(crate::executing::ExecutionEvent::Log(format!("\x1b[1;31mERROR: Failed to add remote to git repository:\n{}\x1b[0m", stderr.trim())));
                                    { let _ = tx.send(crate::executing::ExecutionEvent::Error("Operation failed. Press ESC to go back.".into())); return; }
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(crate::executing::ExecutionEvent::Log(format!("\x1b[1;31mERROR: Failed to execute git remote add: {}\x1b[0m", e)));
                                { let _ = tx.send(crate::executing::ExecutionEvent::Error("Operation failed. Press ESC to go back.".into())); return; }
                            }
                            _ => {}
                        }

                        let ssh_env = if !ssh_key.is_empty() {
                            let expanded_ssh_key = if let Some(stripped) = ssh_key.strip_prefix("~/") {
                                if let Some(home) = directories::UserDirs::new().map(|d| d.home_dir().to_path_buf()) {
                                    home.join(stripped).to_string_lossy().to_string()
                                } else {
                                    ssh_key.to_string()
                                }
                            } else {
                                ssh_key.to_string()
                            };
                            Some(format!("ssh -i {} -o IdentitiesOnly=yes -o StrictHostKeyChecking=accept-new", expanded_ssh_key))
                        } else {
                            None
                        };

                        // Attempt to pull and rebase before pushing to handle existing repos seamlessly
                        let mut pull_cmd = std::process::Command::new("git");
                        pull_cmd
                            .arg("pull")
                            .arg("origin")
                            .arg(initial_branch)
                            .arg("--rebase")
                            .arg("--allow-unrelated-histories")
                            .arg("-X")
                            .arg("theirs")
                            .current_dir(target_dir);

                        if let Some(ref ssh_cmd) = ssh_env {
                            pull_cmd.env("GIT_SSH_COMMAND", ssh_cmd);
                        }

                        // We ignore the result of pull, as it will fail on entirely empty repositories, which is perfectly fine.
                        let _ = pull_cmd.output();

                        let mut push_cmd = std::process::Command::new("git");
                        push_cmd.arg("push").arg("-f").arg("-u").arg("origin").arg(initial_branch).current_dir(target_dir);

                        if let Some(ref ssh_cmd) = ssh_env {
                            push_cmd.env("GIT_SSH_COMMAND", ssh_cmd);
                        }

                        if let Err(e) = run_interactive_cmd(push_cmd, "git push", &tx, &input_rx) {
                            let _ = tx.send(crate::executing::ExecutionEvent::Log(format!("ERROR: {}", e)));
                            let _ = tx.send(crate::executing::ExecutionEvent::Error("Operation failed. Press ESC to go back.".into()));
                            return;
                        }

                        let _ = tx.send(crate::executing::ExecutionEvent::Log(format!(
                            "✓ Git initialized and pushed to remote branch '{}'",
                            initial_branch
                        )));
                    }

                    if bootstrap_flux {
                        let _ = tx.send(crate::executing::ExecutionEvent::Log("[3/3] Bootstrapping Flux...".to_string()));

                        if ssh_key.is_empty() {
                            let _ = tx.send(crate::executing::ExecutionEvent::Log("ERROR: Git SSH Key Path is required for bootstrapping.".to_string()));
                            { let _ = tx.send(crate::executing::ExecutionEvent::Error("Operation failed. Press ESC to go back.".into())); return; }
                        }

                        let kubeconfig_arg = if !kubeconfig.is_empty() {
                            let expanded_kubeconfig = if let Some(stripped) = kubeconfig.strip_prefix("~/") {
                                if let Some(home) = directories::UserDirs::new().map(|d| d.home_dir().to_path_buf()) {
                                    home.join(stripped).to_string_lossy().to_string()
                                } else {
                                    kubeconfig.to_string()
                                }
                            } else {
                                kubeconfig.to_string()
                            };
                            Some(format!("--kubeconfig={}", expanded_kubeconfig))
                        } else {
                            None
                        };

                        let ssh_key_arg = if !ssh_key.is_empty() {
                            let expanded_ssh_key = if let Some(stripped) = ssh_key.strip_prefix("~/") {
                                if let Some(home) = directories::UserDirs::new().map(|d| d.home_dir().to_path_buf()) {
                                    home.join(stripped).to_string_lossy().to_string()
                                } else {
                                    ssh_key.to_string()
                                }
                            } else {
                                ssh_key.to_string()
                            };
                            Some(format!("--private-key-file={}", expanded_ssh_key))
                        } else {
                            None
                        };



                            let mut flux_git_url = git_url.to_string();
                            if flux_git_url.starts_with("git@") && !flux_git_url.starts_with("ssh://")
                                && let Some(colon_pos) = flux_git_url.find(':') {
                                    flux_git_url.replace_range(colon_pos..colon_pos+1, "/");
                                    flux_git_url.insert_str(0, "ssh://");
                                }

                            let mut flux_cmd = std::process::Command::new("flux");
                            flux_cmd
                                .arg("bootstrap")
                                .arg("git")
                                .arg(format!("--url={}", flux_git_url))
                                .arg(format!("--branch={}", branch))
                                .arg(format!("--path={}", path));
                            if git_url.starts_with("http://") {
                                flux_cmd.arg("--allow-insecure-http=true");
                            }

                            if let Some(ref arg) = kubeconfig_arg {
                                flux_cmd.arg(arg);
                            }
                            if let Some(ref arg) = ssh_key_arg {
                                flux_cmd.arg(arg);
                            }

                            if let Err(e) = run_interactive_cmd(flux_cmd, "flux bootstrap", &tx, &input_rx) {
                                let _ = tx.send(crate::executing::ExecutionEvent::Log(format!("ERROR: {}", e)));
                                let _ = tx.send(crate::executing::ExecutionEvent::Error("Operation failed. Press ESC to go back.".into()));
                                return;
                            }
                            let _ = tx.send(crate::executing::ExecutionEvent::Log("✓ Flux bootstrap completed successfully".to_string()));
                    }
                }
            }
        }
    }

        let _ = tx.send(crate::executing::ExecutionEvent::Done);
    });

    app.view = View::Executing(crate::executing::ExecutingState::new(rx, Some(input_tx)));
}
