# pstop

An **htop-like** interactive system monitor for **Windows PowerShell**, written entirely in **Rust**. Designed to be a drop-in replacement for htop on Windows with full feature parity.

## Features

### Display
- **Per-core CPU bars** — two-column layout with multi-color bars (green=user, red=kernel)
- **Memory bar** — green (used), blue (buffers), yellow (cache) — matches htop
- **Swap bar** — color-coded by pressure
- **Tasks line** — process count, thread count, running count
- **Load average** — EMA-approximated (Windows doesn't have native load avg)
- **Uptime** — formatted as `DD days, HH:MM:SS`
- **18-column process table** — `PID PPID USER PRI NI VIRT RES SHR S CPU% MEM% TIME+ THR IO_R IO_W IO_RATE Command`
- **Column header** — cyan background, green highlight on sorted column with ▲/▼
- **Tree view** — Unicode connectors (├─ └─ │) with expand/collapse per-node
- **Search bar** (F3) — jump to match without filtering
- **Filter bar** (F4) — persistent filter that hides non-matching processes
- **F-key bar** — htop-style black-on-cyan key labels

### Advanced Features
- **I/O statistics** — Real-time I/O read/write rates per process (via GetProcessIoCounters)
- **I/O priority** — Set process I/O priority to background mode ('i' key)
- **CPU affinity** — View and modify process CPU affinity masks ('a' key)
- **Process details** — Comprehensive environment/details viewer ('e' key)
- **Open files/handles** — List loaded modules and handles ('l' key - lsof equivalent)
- **Column configuration** — F2 Setup menu to show/hide columns like htop
- **Hide kernel threads** — 'K' key to filter out system/kernel processes
- **PPID column** — Parent process ID tracking
- **Thread count** — THR column showing thread count per process

## Keybindings (htop-compatible)

| Key | Action |
|-----|--------|
| **F1** / **h** / **?** | Help |
| **F2** / **S** | Setup - configure visible columns |
| **F3** / **/** | Search (jump to match) |
| **F4** / **\\** | Filter (hide non-matching) |
| **F5** / **t** | Toggle tree view |
| **F6** | Sort menu |
| **F7** | Nice − (raise priority via Win32) |
| **F8** | Nice + (lower priority via Win32) |
| **F9** / **k** | Kill process (signal menu) |
| **F10** / **q** | Quit |
| ↑ / ↓ / **Alt-k** / **Alt-j** | Navigate |
| PgUp / PgDn / Home / End | Page / jump navigation |
| **P** / **M** / **T** / **N** | Sort by CPU / MEM / TIME / PID |
| **I** | Invert sort order |
| **<** / **>** | Cycle sort column |
| **Space** | Tag process |
| **c** | Tag process + all children |
| **U** | Untag all |
| **u** | Filter by user |
| **a** | Set CPU affinity |
| **e** | Show process details/environment |
| **i** | Set I/O priority (background mode) |
| **l** | List open files/handles (lsof) |
| **F** | Follow selected process |
| **H** | Toggle thread display |
| **K** | Hide kernel/system threads |
| **p** | Toggle full command path |
| **Z** / **z** | Pause / freeze display |
| **Ctrl+L** | Force refresh |
| **+** / **-** / **\*** | Expand / collapse / expand-all tree |
| **0-9** | Quick PID search |
| **Ctrl+C** | Quit |

## Windows-specific

- Real process **priority** and **nice** values via Win32 `GetPriorityClass`
- Real per-process **thread counts** via `CreateToolhelp32Snapshot`
- **I/O statistics** via `GetProcessIoCounters` — read/write bytes per second
- **I/O priority** via `SetPriorityClass` with PROCESS_MODE_BACKGROUND_BEGIN/END modes
- **CPU affinity** via `GetProcessAffinityMask` and `SetProcessAffinityMask`
- **Open handles** enumeration via `EnumProcessModulesEx` and `GetModuleFileNameExW`
- Priority changes via `SetPriorityClass` (F7/F8)
- Process kill via `taskkill /F`
- User resolution from Windows SIDs
- PPID (Parent PID) tracking via sysinfo

## Building

```powershell
cargo build --release
```

The binary will be at `target/release/pstop.exe` (~800 KB).

## Running

```powershell
.\target\release\pstop.exe
```

## Project Structure

```
src/
├── main.rs                # Entry point, terminal setup, main loop
├── app.rs                 # Application state, modes, sorting, filtering, tree
├── input.rs               # Keyboard input handling for all modes
├── system/
│   ├── mod.rs
│   ├── cpu.rs             # CPU core & aggregate info structs
│   ├── memory.rs          # Memory/swap info & byte formatting
│   ├── process.rs         # ProcessInfo, ProcessStatus, ProcessSortField (17 fields)
│   ├── collector.rs       # System data collection (sysinfo + Win32 + I/O)
│   └── winapi.rs          # Win32 API: priority, threads, I/O, affinity, handles
└── ui/
    ├── mod.rs             # UI layout & draw dispatcher
    ├── header.rs          # CPU bars, memory bars, tasks/load/uptime
    ├── process_table.rs   # Process list with 17-column headers
    ├── footer.rs          # F-key function bar
    ├── help.rs            # Help overlay popup
    ├── sort_menu.rs       # Sort-by selection overlay
    ├── kill_menu.rs       # Kill signal selection overlay
    ├── user_menu.rs       # User filter selection overlay
    ├── affinity_menu.rs   # CPU affinity selection overlay
    ├── environment_view.rs # Process details/environment viewer
    ├── setup_menu.rs      # F2 column configuration overlay
    └── handles_view.rs    # Open files/handles viewer (lsof)
```

## Dependencies

- **[ratatui](https://ratatui.rs)** — TUI rendering framework
- **[crossterm](https://github.com/crossterm-rs/crossterm)** — Terminal backend
- **[sysinfo](https://github.com/GuillaumeGomez/sysinfo)** — System data collection
- **[windows](https://github.com/microsoft/windows-rs)** — Win32 API bindings

## License

MIT
