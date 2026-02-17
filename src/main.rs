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
pub mod color_scheme;
mod config;
mod input;
mod mouse;
mod system;
mod ui;

use std::io::{self, BufWriter};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute, queue,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::App;
use system::collector::Collector;

/// Refresh interval in milliseconds
const TICK_RATE_MS: u64 = 1500;

fn main() -> Result<()> {
    // Handle CLI flags before entering TUI mode
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "--install-alias" => {
                return install_htop_alias();
            }
            "--compact" | "-c" => {
                // Compact mode handled below during app init
            }
            "--help" | "-h" => {
                println!("pstop — An htop-like system monitor for Windows");
                println!();
                println!("Usage: pstop [OPTIONS]");
                println!();
                println!("Options:");
                println!("  --compact, -c     Compact mode (minimal header, ideal for small screens/mobile)");
                println!("  --install-alias   Add 'htop' alias to your PowerShell profile");
                println!("  --help, -h        Show this help message");
                return Ok(());
            }
            _ => {
                eprintln!("Unknown option: {}", args[1]);
                eprintln!("Run 'pstop --help' for usage information.");
                std::process::exit(1);
            }
        }
    }

    let compact = args.iter().any(|a| a == "--compact" || a == "-c");

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    // Wrap stdout in BufWriter to batch escape sequences into fewer write syscalls,
    // significantly reducing flicker when running inside terminal multiplexers.
    let buffered = BufWriter::with_capacity(16384, stdout);
    let backend = CrosstermBackend::new(buffered);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Run the app
    let result = run_app(&mut terminal, compact);

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
fn run_app(terminal: &mut Terminal<CrosstermBackend<BufWriter<io::Stdout>>>, compact: bool) -> Result<()> {
    let mut app = App::new();
    app.compact_mode = compact;
    let mut collector = Collector::new();

    // Load saved configuration
    let cfg = config::PstopConfig::load();
    cfg.apply_to(&mut app);

    let mut last_tick = Instant::now();

    // Initial data collection
    collector.refresh(&mut app);

    loop {
        // Update visible rows based on terminal size
        let size = terminal.size()?;
        let header_h = ui::header_height(&app) as usize;
        let footer_h = 1;
        let available = size.height as usize;
        // Account for search/filter bar stealing 1 row from process area
        let bar_h: usize = if app.mode == app::AppMode::Search
            || app.mode == app::AppMode::Filter
            || !app.filter_query.is_empty()
        { 1 } else { 0 };
        app.visible_rows = if available > header_h + footer_h + 2 + bar_h {
            available - header_h - footer_h - 2 - bar_h // -2 for table header + tab bar
        } else {
            5
        };

        // Wrap the draw in synchronized output to prevent flicker inside
        // terminal multiplexers (psmux, tmux, etc.).
        use std::io::Write;
        queue!(terminal.backend_mut(), crossterm::terminal::BeginSynchronizedUpdate)?;
        terminal.draw(|f| ui::draw(f, &app))?;
        queue!(terminal.backend_mut(), crossterm::terminal::EndSynchronizedUpdate)?;
        terminal.backend_mut().flush()?;

        // Check if we should quit before waiting for events
        if app.should_quit {
            // Save configuration on quit
            let _ = config::PstopConfig::from_app(&app).save();
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
                            let _ = config::PstopConfig::from_app(&app).save();
                            return Ok(());
                        }
                    }
                }
                Event::Mouse(mouse_event) => {
                    if app.enable_mouse {
                        mouse::handle_mouse(&mut app, mouse_event, size.width, size.height);
                        if app.should_quit {
                            let _ = config::PstopConfig::from_app(&app).save();
                            return Ok(());
                        }
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
        let dynamic_tick = Duration::from_millis(app.update_interval_ms);
        if now.duration_since(last_tick) >= dynamic_tick {
            should_refresh = true;
            last_tick = now;
        }

        if should_refresh {
            collector.refresh(&mut app);
        }
    }
}

/// Install 'htop' alias for pstop in the user's PowerShell profile.
/// Writes `Set-Alias htop pstop` to the profile file, creating it if needed.
fn install_htop_alias() -> Result<()> {
    use std::fs;
    use std::path::PathBuf;

    // Get the PowerShell profile path via $PROFILE
    let output = std::process::Command::new("pwsh")
        .args(["-NoProfile", "-Command", "echo $PROFILE"])
        .output()
        .or_else(|_| {
            // Fall back to powershell.exe (Windows PowerShell 5.x)
            std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", "echo $PROFILE"])
                .output()
        })?;

    let profile_path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if profile_path_str.is_empty() {
        anyhow::bail!("Could not determine PowerShell profile path. Is PowerShell installed?");
    }

    let profile_path = PathBuf::from(&profile_path_str);
    let alias_line = "Set-Alias htop pstop";

    // Check if alias already exists in profile
    if profile_path.exists() {
        let content = fs::read_to_string(&profile_path)?;
        if content.contains(alias_line) {
            println!("✓ 'htop' alias already exists in {}", profile_path_str);
            return Ok(());
        }
    } else {
        // Create parent directories if needed
        if let Some(parent) = profile_path.parent() {
            fs::create_dir_all(parent)?;
        }
    }

    // Append the alias to the profile
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&profile_path)?;
    writeln!(file)?; // blank line separator
    writeln!(file, "# pstop: htop alias for Windows")?;
    writeln!(file, "{}", alias_line)?;

    println!("✓ Added 'htop' alias to {}", profile_path_str);
    println!("  Restart PowerShell or run: . $PROFILE");

    Ok(())
}
