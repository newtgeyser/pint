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
use app::App;

pub fn run() -> Result<()> {
    // Initialize database
    let conn = db::open()?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

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

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Esc => {
                    if app.is_searching() {
                        app.cancel_search();
                    } else {
                        return Ok(());
                    }
                }
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
                KeyCode::Char('s') => {
                    if !app.is_searching() {
                        app.sync()?;
                    } else {
                        app.search_input('s');
                    }
                }
                KeyCode::Char('r') => {
                    if !app.is_searching() {
                        app.load_data()?;
                    } else {
                        app.search_input('r');
                    }
                }
                KeyCode::Char(c) => {
                    if app.is_searching() {
                        app.search_input(c);
                    }
                }
                KeyCode::Backspace => {
                    if app.is_searching() {
                        app.search_backspace();
                    }
                }
                _ => {}
            }
        }
    }
}
