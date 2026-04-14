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
    let startup_time = Instant::now();

    // Start Collector initialization ASAP — the ~182ms sysinfo CPU init
    // runs concurrently with CLI parsing, terminal setup, and first frame render.
    // For non-TUI paths (--help, --bench), this thread is dropped harmlessly.
    let collector_handle = std::thread::spawn(Collector::new);

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
            "--bench" => {
                // Benchmark mode: measure startup time and exit
                // Drop the eagerly-spawned collector — benchmark creates its own
                drop(collector_handle);
                return run_benchmark();
            }
            "--help" | "-h" => {
                println!("pstop — An htop-like system monitor for Windows");
                println!();
                println!("Usage: pstop [OPTIONS]");
                println!();
                println!("Options:");
                println!("  --compact, -c     Compact mode (minimal header, ideal for small screens/mobile)");
                println!("  --bench           Benchmark startup time and exit");
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
    let result = run_app(&mut terminal, compact, startup_time, collector_handle);

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

    // Force-exit to kill any lingering background threads (collector
    // batch threads for I/O counters, process data, etc.).
    std::process::exit(0);
}

/// Main application loop
fn run_app(terminal: &mut Terminal<CrosstermBackend<BufWriter<io::Stdout>>>, compact: bool, startup_time: Instant, collector_handle: std::thread::JoinHandle<Collector>) -> Result<()> {
    let mut app = App::new();
    app.compact_mode = compact;

    // Load saved configuration (fast file I/O, < 1ms)
    let cfg = config::PstopConfig::load();
    cfg.apply_to(&mut app);

    // Collector initialization was already spawned at the very start of main(),
    // maximizing overlap with terminal setup + first frame render.

    // ── Instant first frame: render the UI skeleton before any data collection ──
    // This makes the app appear immediately while system queries run.
    {
        let size = terminal.size()?;
        let header_h = ui::header_height(&app, size.height, size.width) as usize;
        let footer_h = 1;
        let available = size.height as usize;
        app.visible_rows = if available > header_h + footer_h + 2 { available - header_h - footer_h - 2 } else { 5 };

        use std::io::Write;
        queue!(terminal.backend_mut(), crossterm::terminal::BeginSynchronizedUpdate)?;
        terminal.draw(|f| ui::draw(f, &app))?;
        queue!(terminal.backend_mut(), crossterm::terminal::EndSynchronizedUpdate)?;
        terminal.backend_mut().flush()?;
    }

    let first_frame_ms = startup_time.elapsed().as_millis();

    // Wait for collector init (most of the ~165ms already elapsed during frame render)
    let mut collector = collector_handle.join().expect("Collector init panicked");

    // ── Second frame: CPU bars + memory bars appear before process enumeration ──
    // refresh_header_only() takes ~2ms vs 120ms for full refresh. This gives the
    // user a responsive frame with live CPU/memory bars while processes load.
    {
        collector.refresh_header_only(&mut app);
        use std::io::Write;
        queue!(terminal.backend_mut(), crossterm::terminal::BeginSynchronizedUpdate)?;
        terminal.draw(|f| ui::draw(f, &app))?;
        queue!(terminal.backend_mut(), crossterm::terminal::EndSynchronizedUpdate)?;
        terminal.backend_mut().flush()?;
    }

    // Full refresh populates process table (~120ms)
    collector.refresh(&mut app);

    // Store startup timing for display
    app.startup_first_frame_ms = first_frame_ms as u64;
    app.startup_fully_loaded_ms = startup_time.elapsed().as_millis() as u64;

    let mut last_tick = Instant::now();

    loop {
        // Update visible rows based on terminal size
        let size = terminal.size()?;
        let header_h = ui::header_height(&app, size.height, size.width) as usize;
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

/// Benchmark startup time: initialize everything, render one full frame, and print timing.
fn run_benchmark() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let verbose = args.iter().any(|a| a == "-v" || a == "--verbose");

    let t0 = Instant::now();

    let mut app = App::new();
    let cfg = config::PstopConfig::load();
    cfg.apply_to(&mut app);
    let t_app = t0.elapsed();

    if verbose {
        // Detailed sub-component timing for Collector::new()
        let tc0 = Instant::now();
        let boot_handle = std::thread::spawn(|| crate::system::winapi::get_real_boot_time());
        let tc_boot_spawn = tc0.elapsed();

        let mut sys = sysinfo::System::new();
        let tc_sys_new = tc0.elapsed();

        sys.refresh_cpu_all();
        let tc_cpu1 = tc0.elapsed();

        sys.refresh_memory();
        let tc_mem = tc0.elapsed();

        std::thread::sleep(std::time::Duration::from_millis(10));
        let tc_sleep = tc0.elapsed();

        sys.refresh_cpu_all();
        let tc_cpu2 = tc0.elapsed();

        let networks = sysinfo::Networks::new_with_refreshed_list();
        let tc_net = tc0.elapsed();

        drop((sys, networks, boot_handle));
        let t_collector = t0.elapsed();

        println!("pstop startup benchmark (verbose):");
        println!("  App::new() + config:  {:>6.1}ms", t_app.as_secs_f64() * 1000.0);
        println!();
        println!("  Collector::new() breakdown:");
        println!("    spawn boot thread:  {:>6.1}ms", tc_boot_spawn.as_secs_f64() * 1000.0);
        println!("    System::new():      {:>6.1}ms", (tc_sys_new - tc_boot_spawn).as_secs_f64() * 1000.0);
        println!("    refresh_cpu_all #1: {:>6.1}ms", (tc_cpu1 - tc_sys_new).as_secs_f64() * 1000.0);
        println!("    refresh_memory:     {:>6.1}ms", (tc_mem - tc_cpu1).as_secs_f64() * 1000.0);
        println!("    sleep(10ms):        {:>6.1}ms", (tc_sleep - tc_mem).as_secs_f64() * 1000.0);
        println!("    refresh_cpu_all #2: {:>6.1}ms", (tc_cpu2 - tc_sleep).as_secs_f64() * 1000.0);
        println!("    Networks::new():    {:>6.1}ms", (tc_net - tc_cpu2).as_secs_f64() * 1000.0);
        println!("    total new():        {:>6.1}ms", (t_collector - t_app).as_secs_f64() * 1000.0);
        println!();

        // Detailed sub-component timing for refresh()
        let mut collector = Collector::new();
        let tr0 = Instant::now();

        let pids = crate::system::winapi::quick_enumerate_pids();
        let tr_enum = tr0.elapsed();

        let pids2 = pids.clone();
        let io_h = std::thread::spawn(move || crate::system::winapi::batch_io_counters(&pids2));
        let pids3 = pids.clone();
        let data_h = std::thread::spawn(move || crate::system::winapi::collect_process_data(&pids3));
        let pids4 = pids.clone();
        let users_h = std::thread::spawn(move || crate::system::winapi::batch_process_users(&pids4));
        let pids5 = pids;
        let times_h = std::thread::spawn(move || crate::system::winapi::batch_process_times(&pids5));
        let tr_spawn = tr0.elapsed();

        // These run while Win32 threads are in-flight
        collector.sys.refresh_cpu_all();
        let tr_cpu = tr0.elapsed();
        collector.sys.refresh_memory();
        let tr_mem2 = tr0.elapsed();
        collector.sys.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            true,
            sysinfo::ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_cmd(sysinfo::UpdateKind::OnlyIfNotSet),
        );
        let tr_procs = tr0.elapsed();

        let _io = io_h.join().unwrap_or_default();
        let tr_io_join = tr0.elapsed();
        let _data = data_h.join().unwrap_or_default();
        let tr_data_join = tr0.elapsed();
        let _users = users_h.join().unwrap_or_default();
        let tr_users_join = tr0.elapsed();
        let _times = times_h.join().unwrap_or_default();
        let tr_times_join = tr0.elapsed();

        println!("  refresh() breakdown:");
        println!("    quick_enumerate:    {:>6.1}ms", tr_enum.as_secs_f64() * 1000.0);
        println!("    spawn Win32 thrds:  {:>6.1}ms", (tr_spawn - tr_enum).as_secs_f64() * 1000.0);
        println!("    refresh_cpu_all:    {:>6.1}ms", (tr_cpu - tr_spawn).as_secs_f64() * 1000.0);
        println!("    refresh_memory:     {:>6.1}ms", (tr_mem2 - tr_cpu).as_secs_f64() * 1000.0);
        println!("    refresh_processes:  {:>6.1}ms", (tr_procs - tr_mem2).as_secs_f64() * 1000.0);
        println!("    join IO:            {:>6.1}ms (wait after sysinfo)", (tr_io_join - tr_procs).as_secs_f64() * 1000.0);
        println!("    join data:          {:>6.1}ms", (tr_data_join - tr_io_join).as_secs_f64() * 1000.0);
        println!("    join users:         {:>6.1}ms", (tr_users_join - tr_data_join).as_secs_f64() * 1000.0);
        println!("    join times:         {:>6.1}ms", (tr_times_join - tr_users_join).as_secs_f64() * 1000.0);
        println!("    total refresh:      {:>6.1}ms", tr_times_join.as_secs_f64() * 1000.0);
        println!();

        // Collect processes timing
        let tc0 = Instant::now();
        collector.refresh(&mut app);
        let tc_full = tc0.elapsed();
        println!("  Full refresh() call:  {:>6.1}ms", tc_full.as_secs_f64() * 1000.0);
        println!("  Processes: {}, CPU cores: {}", app.processes.len(), app.cpu_info.logical_cores);
    } else {
        let mut collector = Collector::new();
        let t_collector = t0.elapsed();

        collector.refresh(&mut app);
        let t_refresh = t0.elapsed();

        let t_total = t0.elapsed();

        println!("pstop startup benchmark:");
        println!("  App::new() + config:  {:>6.1}ms", t_app.as_secs_f64() * 1000.0);
        println!("  Collector::new():     {:>6.1}ms", (t_collector - t_app).as_secs_f64() * 1000.0);
        println!("  First refresh():      {:>6.1}ms", (t_refresh - t_collector).as_secs_f64() * 1000.0);
        println!("  ─────────────────────────────");
        println!("  Total to data ready:  {:>6.1}ms", t_total.as_secs_f64() * 1000.0);
        println!("  First frame (UI):     < 1ms (skeleton rendered before data)");
        println!();
        println!("  Processes enumerated: {}", app.processes.len());
        println!("  CPU cores:            {}", app.cpu_info.logical_cores);
    }

    Ok(())
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
