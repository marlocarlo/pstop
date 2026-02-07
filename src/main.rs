//! pstop — An htop-like system monitor for Windows, written in Rust.
//!
//! Features:
//!   - Per-core CPU usage bars
//!   - Memory & swap usage bars
//!   - Full process table with sorting
//!   - Search / filter processes
//!   - Tree view
//!   - Kill processes
//!   - htop-style F-key bar & color scheme
//!
//! Keybindings: Press F1 or '?' for help.

#![allow(dead_code)]

mod app;
mod input;
mod system;
mod ui;

use std::io;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::App;
use system::collector::Collector;

/// Refresh interval in milliseconds
const TICK_RATE_MS: u64 = 1500;

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Run the app
    let result = run_app(&mut terminal);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}

/// Main application loop
fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut app = App::new();
    let mut collector = Collector::new();
    let tick_rate = Duration::from_millis(TICK_RATE_MS);
    let mut last_tick = Instant::now();

    // Initial data collection
    collector.refresh(&mut app);

    loop {
        // Update visible rows based on terminal size
        let size = terminal.size()?;
        let header_h = ui::header_height(&app) as usize;
        let footer_h = 1;
        let available = size.height as usize;
        app.visible_rows = if available > header_h + footer_h + 2 {
            available - header_h - footer_h - 2 // -2 for table header + borders
        } else {
            5
        };

        // Draw
        terminal.draw(|f| ui::draw(f, &app))?;

        // Check if we should quit before waiting for events
        if app.should_quit {
            return Ok(());
        }

        // Handle events with short timeout for responsiveness
        let timeout = Duration::from_millis(50);
        let mut should_refresh = false;

        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    // On Windows, crossterm fires Press and Release; only handle Press
                    if key.kind == KeyEventKind::Press {
                        input::handle_input(&mut app, key);
                        // Immediate redraw after user input for responsiveness
                        if app.should_quit {
                            return Ok(());
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    use crossterm::event::{MouseEventKind};
                    match mouse.kind {
                        MouseEventKind::ScrollUp => app.select_prev(),
                        MouseEventKind::ScrollDown => app.select_next(),
                        _ => {}
                    }
                }
                Event::Resize(_, _) => {
                    // Terminal resize - will be handled on next draw
                }
                _ => {}
            }
        }

        // Check if it's time to refresh system data
        let now = Instant::now();
        if now.duration_since(last_tick) >= tick_rate {
            should_refresh = true;
            last_tick = now;
        }

        if should_refresh {
            collector.refresh(&mut app);
        }
    }
}
