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
const TICK_RATE_MS: u64 = 1000;

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

        // Handle events with timeout = tick_rate
        let timeout = tick_rate;
        let deadline = Instant::now() + timeout;

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }

            if event::poll(remaining)? {
                match event::read()? {
                    Event::Key(key) => {
                        // On Windows, crossterm fires Press and Release; only handle Press
                        if key.kind == KeyEventKind::Press {
                            input::handle_input(&mut app, key);
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
                    _ => {}
                }
            } else {
                break;
            }

            if app.should_quit {
                return Ok(());
            }
        }

        if app.should_quit {
            return Ok(());
        }

        // Refresh system data
        collector.refresh(&mut app);
    }
}
