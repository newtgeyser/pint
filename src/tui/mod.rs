mod app;
mod ui;

use std::io;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

use crate::db;
use app::{App, View};

pub fn run() -> Result<()> {
    // Initialize database
    let conn = db::open()?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Create app state
    let mut app = App::new(conn);
    app.load_data()?;

    // Main loop
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;

        // Handle pending sync after render (so dialog is visible)
        if app.pending_sync {
            app.execute_sync();
            continue;
        }

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            // Clear status message on any key press
            app.status = None;

            // Mark as interacted after first key
            if !app.has_interacted {
                app.has_interacted = true;
            }

            // Handle dialog mode first
            if app.has_dialog() {
                match key.code {
                    KeyCode::Esc => app.close_dialog(),
                    KeyCode::Enter => app.dialog_confirm()?,
                    KeyCode::Tab => app.dialog_next_field(),
                    KeyCode::Backspace => app.dialog_backspace(),
                    KeyCode::Left => app.dialog_cursor_left(),
                    KeyCode::Right => app.dialog_cursor_right(),
                    KeyCode::Up => app.dialog_select_up(),
                    KeyCode::Down => app.dialog_select_down(),
                    KeyCode::Char(c) => app.dialog_input(c),
                    _ => {}
                }
                continue;
            }

            // Handle search mode
            if app.is_searching() {
                match key.code {
                    KeyCode::Esc => app.cancel_search(),
                    KeyCode::Enter => {
                        app.search_mode = false;
                        // Keep the search query active
                    }
                    KeyCode::Backspace => app.search_backspace(),
                    KeyCode::Char(c) => app.search_input(c),
                    _ => {}
                }
                continue;
            }

            // Normal mode - handle navigation and view-specific keys
            match key.code {
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Esc => return Ok(()),
                KeyCode::Tab => app.toggle_focus(),
                KeyCode::Up | KeyCode::Char('k') => {
                    if app.nav_focused {
                        app.nav_up();
                    } else {
                        app.list_up();
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if app.nav_focused {
                        app.nav_down();
                    } else {
                        app.list_down();
                    }
                }
                KeyCode::PageUp => {
                    if !app.nav_focused {
                        app.list_page_up();
                    }
                }
                KeyCode::PageDown => {
                    if !app.nav_focused {
                        app.list_page_down();
                    }
                }
                KeyCode::Enter => {
                    if app.nav_focused {
                        app.select_nav();
                        app.load_data()?;
                    } else {
                        app.select_item()?;
                    }
                }
                KeyCode::Char('/') => {
                    if !app.nav_focused {
                        app.start_search();
                    }
                }
                // View-specific keys (only when not in nav)
                _ if !app.nav_focused => {
                    handle_view_keys(app, key.code)?;
                }
                _ => {}
            }
        }
    }
}

fn handle_view_keys(app: &mut App, key: KeyCode) -> Result<()> {
    match app.current_view {
        View::Summary => match key {
            KeyCode::Char('s') => app.start_sync(),
            _ => {}
        },
        View::Accounts => match key {
            KeyCode::Char('a') => app.show_add_account_dialog(),
            KeyCode::Char('d') => app.show_remove_account_dialog(),
            KeyCode::Char('r') => app.show_rename_account_dialog(),
            KeyCode::Char('t') => app.show_set_type_dialog(),
            KeyCode::Char('s') => app.start_sync(),
            _ => {}
        },
        View::Transactions => match key {
            KeyCode::Char('f') => app.show_filter_account_dialog(),
            KeyCode::Char('c') => app.clear_filters()?,
            KeyCode::Char('l') => app.show_rule_dialog(),
            KeyCode::Char('e') => app.show_categorize_dialog(),
            KeyCode::Char('r') => app.show_reimburse_dialog(),
            KeyCode::Char('p') => app.toggle_reimbursed_paid()?,
            _ => {}
        },
        View::Holdings => match key {
            KeyCode::Char('e') | KeyCode::Enter => app.show_edit_holding_dialog(),
            _ => {}
        }
        View::Recurring => {
            // Recurring-specific keys can be added here
        }
        View::Assets => match key {
            KeyCode::Char('a') => app.show_add_asset_dialog(),
            KeyCode::Char('d') => {
                app.status = Some("Asset delete not yet implemented in TUI".to_string());
            }
            KeyCode::Char('e') => {
                app.status = Some("Asset edit not yet implemented in TUI".to_string());
            }
            _ => {}
        },
        View::Rules => match key {
            KeyCode::Char('a') => {
                crate::commands::categorize::run_auto_quiet(true)?;
                app.load_data()?;
                app.status = Some("Rules applied to transactions".to_string());
            }
            KeyCode::Char('e') | KeyCode::Enter => {
                app.show_edit_rule_from_list();
            }
            KeyCode::Char('c') => {
                app.clear_filters()?;
            }
            _ => {}
        },
    }
    Ok(())
}
