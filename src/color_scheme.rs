use ratatui::style::{Color, Modifier, Style};

/// All htop color scheme variants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSchemeId {
    Default = 0,
    Monochrome = 1,
    BlackNight = 2,
    LightTerminal = 3,
    MidnightCommander = 4,
    BlackOnWhite = 5,
    DarkVivid = 6,
}

impl ColorSchemeId {
    pub fn all() -> &'static [ColorSchemeId] {
        &[
            ColorSchemeId::Default,
            ColorSchemeId::Monochrome,
            ColorSchemeId::BlackNight,
            ColorSchemeId::LightTerminal,
            ColorSchemeId::MidnightCommander,
            ColorSchemeId::BlackOnWhite,
            ColorSchemeId::DarkVivid,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            ColorSchemeId::Default => "Default",
            ColorSchemeId::Monochrome => "Monochrome",
            ColorSchemeId::BlackNight => "Black Night",
            ColorSchemeId::LightTerminal => "Light Terminal",
            ColorSchemeId::MidnightCommander => "MC",
            ColorSchemeId::BlackOnWhite => "Black on White",
            ColorSchemeId::DarkVivid => "Dark Vivid",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ColorSchemeId::Default => "htop classic dark theme",
            ColorSchemeId::Monochrome => "No colors, monochrome",
            ColorSchemeId::BlackNight => "Dark theme for dark terminals",
            ColorSchemeId::LightTerminal => "For light terminal backgrounds",
            ColorSchemeId::MidnightCommander => "Midnight Commander style",
            ColorSchemeId::BlackOnWhite => "Black text on white background",
            ColorSchemeId::DarkVivid => "Vivid dark colors with contrast",
        }
    }

    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => ColorSchemeId::Default,
            1 => ColorSchemeId::Monochrome,
            2 => ColorSchemeId::BlackNight,
            3 => ColorSchemeId::LightTerminal,
            4 => ColorSchemeId::MidnightCommander,
            5 => ColorSchemeId::BlackOnWhite,
            6 => ColorSchemeId::DarkVivid,
            _ => ColorSchemeId::Default,
        }
    }
}

/// All configurable color slots used across the app
#[derive(Debug, Clone)]
pub struct ColorScheme {
    // Background
    pub bg: Color,

    // Header bars
    pub cpu_bar_normal: Color,     // User/normal processes
    pub cpu_bar_system: Color,     // Kernel/system
    pub cpu_bar_low: Color,        // Low priority (nice > 0)
    pub cpu_bar_virt: Color,       // Virtual/steal/guest
    pub cpu_bar_iowait: Color,     // IO wait (detailed mode)
    pub cpu_bar_irq: Color,        // IRQ (detailed mode)
    pub cpu_label: Color,          // CPU number label
    pub cpu_bar_bg: Color,         // Bar background char color

    pub mem_bar_used: Color,       // Used memory
    pub mem_bar_buffers: Color,    // Buffers
    pub mem_bar_cache: Color,      // Cache

    pub swap_bar: Color,           // Swap usage

    // Header info text
    pub tasks_text: Color,
    pub load_text: Color,
    pub uptime_text: Color,
    pub info_label: Color,         // "Tasks:", "Load average:" labels
    pub info_value: Color,         // Values/numbers

    // Process table header
    pub table_header_bg: Color,
    pub table_header_fg: Color,
    pub table_header_sort_bg: Color,
    pub table_header_sort_fg: Color,

    // Process table rows
    pub process_fg: Color,          // Default process text
    pub process_bg: Color,          // Default row background
    pub process_selected_bg: Color, // Selected row background
    pub process_selected_fg: Color, // Selected row fg
    pub process_shadow: Color,      // Other users' processes (shadow)

    // Column-specific colors
    pub col_pid: Color,
    pub col_user: Color,
    pub col_priority: Color,
    pub col_mem_high: Color,       // highlight_megabytes color
    pub col_mem_normal: Color,
    pub col_cpu_high: Color,       // >75%
    pub col_cpu_medium: Color,     // >25%
    pub col_cpu_low: Color,        // <=25%
    pub col_status_running: Color,
    pub col_status_sleeping: Color,
    pub col_status_disk_sleep: Color,
    pub col_status_stopped: Color,
    pub col_status_zombie: Color,
    pub col_status_unknown: Color,
    pub col_command: Color,
    pub col_command_basename: Color, // Highlighted base name
    pub col_thread: Color,          // Thread color

    // Footer
    pub footer_key_fg: Color,
    pub footer_key_bg: Color,
    pub footer_label_fg: Color,
    pub footer_label_bg: Color,

    // Tab bar
    pub tab_active_bg: Color,
    pub tab_active_fg: Color,
    pub tab_inactive_fg: Color,
    pub tab_inactive_bg: Color,

    // Popups / menus
    pub popup_border: Color,
    pub popup_bg: Color,
    pub popup_title: Color,
    pub popup_selected_bg: Color,
    pub popup_selected_fg: Color,
    pub popup_text: Color,

    // Search/filter bar
    pub search_label: Color,
    pub search_text: Color,
    pub filter_label: Color,
    pub filter_text: Color,
}

impl ColorScheme {
    pub fn from_id(id: ColorSchemeId) -> Self {
        match id {
            ColorSchemeId::Default => Self::default_scheme(),
            ColorSchemeId::Monochrome => Self::monochrome(),
            ColorSchemeId::BlackNight => Self::black_night(),
            ColorSchemeId::LightTerminal => Self::light_terminal(),
            ColorSchemeId::MidnightCommander => Self::midnight_commander(),
            ColorSchemeId::BlackOnWhite => Self::black_on_white(),
            ColorSchemeId::DarkVivid => Self::dark_vivid(),
        }
    }

    /// htop Default color scheme
    fn default_scheme() -> Self {
        Self {
            bg: Color::Reset,

            cpu_bar_normal: Color::Green,
            cpu_bar_system: Color::Red,
            cpu_bar_low: Color::Blue,
            cpu_bar_virt: Color::Cyan,
            cpu_bar_iowait: Color::DarkGray,
            cpu_bar_irq: Color::Magenta,
            cpu_label: Color::White,
            cpu_bar_bg: Color::DarkGray,

            mem_bar_used: Color::Green,
            mem_bar_buffers: Color::Blue,
            mem_bar_cache: Color::Yellow,

            swap_bar: Color::Red,

            tasks_text: Color::White,
            load_text: Color::White,
            uptime_text: Color::White,
            info_label: Color::White,
            info_value: Color::Cyan,

            table_header_bg: Color::Cyan,
            table_header_fg: Color::Black,
            table_header_sort_bg: Color::Green,
            table_header_sort_fg: Color::Black,

            process_fg: Color::White,
            process_bg: Color::Reset,
            process_selected_bg: Color::Indexed(236),
            process_selected_fg: Color::White,
            process_shadow: Color::DarkGray,

            col_pid: Color::Green,
            col_user: Color::White,
            col_priority: Color::White,
            col_mem_high: Color::Green,
            col_mem_normal: Color::White,
            col_cpu_high: Color::Red,
            col_cpu_medium: Color::Yellow,
            col_cpu_low: Color::Green,
            col_status_running: Color::Green,
            col_status_sleeping: Color::Reset,
            col_status_disk_sleep: Color::Red,
            col_status_stopped: Color::Red,
            col_status_zombie: Color::Magenta,
            col_status_unknown: Color::DarkGray,
            col_command: Color::White,
            col_command_basename: Color::Green,
            col_thread: Color::Blue,

            footer_key_fg: Color::Black,
            footer_key_bg: Color::Cyan,
            footer_label_fg: Color::Cyan,
            footer_label_bg: Color::Black,

            tab_active_bg: Color::Cyan,
            tab_active_fg: Color::Black,
            tab_inactive_fg: Color::DarkGray,
            tab_inactive_bg: Color::Indexed(235),

            popup_border: Color::Cyan,
            popup_bg: Color::Black,
            popup_title: Color::Cyan,
            popup_selected_bg: Color::Cyan,
            popup_selected_fg: Color::Black,
            popup_text: Color::White,

            search_label: Color::Cyan,
            search_text: Color::White,
            filter_label: Color::Yellow,
            filter_text: Color::White,
        }
    }

    /// Monochrome (no colors)
    fn monochrome() -> Self {
        Self {
            bg: Color::Reset,

            cpu_bar_normal: Color::White,
            cpu_bar_system: Color::White,
            cpu_bar_low: Color::White,
            cpu_bar_virt: Color::White,
            cpu_bar_iowait: Color::White,
            cpu_bar_irq: Color::White,
            cpu_label: Color::White,
            cpu_bar_bg: Color::DarkGray,

            mem_bar_used: Color::White,
            mem_bar_buffers: Color::White,
            mem_bar_cache: Color::White,

            swap_bar: Color::White,

            tasks_text: Color::White,
            load_text: Color::White,
            uptime_text: Color::White,
            info_label: Color::White,
            info_value: Color::White,

            table_header_bg: Color::White,
            table_header_fg: Color::Black,
            table_header_sort_bg: Color::White,
            table_header_sort_fg: Color::Black,

            process_fg: Color::White,
            process_bg: Color::Reset,
            process_selected_bg: Color::White,
            process_selected_fg: Color::Black,
            process_shadow: Color::DarkGray,

            col_pid: Color::White,
            col_user: Color::White,
            col_priority: Color::White,
            col_mem_high: Color::White,
            col_mem_normal: Color::White,
            col_cpu_high: Color::White,
            col_cpu_medium: Color::White,
            col_cpu_low: Color::White,
            col_status_running: Color::White,
            col_status_sleeping: Color::White,
            col_status_disk_sleep: Color::White,
            col_status_stopped: Color::White,
            col_status_zombie: Color::White,
            col_status_unknown: Color::DarkGray,
            col_command: Color::White,
            col_command_basename: Color::White,
            col_thread: Color::White,

            footer_key_fg: Color::Black,
            footer_key_bg: Color::White,
            footer_label_fg: Color::White,
            footer_label_bg: Color::Reset,

            tab_active_bg: Color::White,
            tab_active_fg: Color::Black,
            tab_inactive_fg: Color::DarkGray,
            tab_inactive_bg: Color::Indexed(235),

            popup_border: Color::White,
            popup_bg: Color::Black,
            popup_title: Color::White,
            popup_selected_bg: Color::White,
            popup_selected_fg: Color::Black,
            popup_text: Color::White,

            search_label: Color::White,
            search_text: Color::White,
            filter_label: Color::White,
            filter_text: Color::White,
        }
    }

    /// Black Night — dark theme, similar to default but adjusted for dark terminals
    fn black_night() -> Self {
        Self {
            bg: Color::Black,

            cpu_bar_normal: Color::Green,
            cpu_bar_system: Color::Red,
            cpu_bar_low: Color::Indexed(33),   // Bright blue
            cpu_bar_virt: Color::Cyan,
            cpu_bar_iowait: Color::Indexed(245),
            cpu_bar_irq: Color::Magenta,
            cpu_label: Color::Indexed(250),
            cpu_bar_bg: Color::Indexed(238),

            mem_bar_used: Color::Green,
            mem_bar_buffers: Color::Indexed(33),
            mem_bar_cache: Color::Yellow,

            swap_bar: Color::Red,

            tasks_text: Color::Indexed(250),
            load_text: Color::Indexed(250),
            uptime_text: Color::Indexed(250),
            info_label: Color::Indexed(250),
            info_value: Color::Cyan,

            table_header_bg: Color::Indexed(238),
            table_header_fg: Color::Indexed(250),
            table_header_sort_bg: Color::Green,
            table_header_sort_fg: Color::Black,

            process_fg: Color::Indexed(250),
            process_bg: Color::Black,
            process_selected_bg: Color::Indexed(236),
            process_selected_fg: Color::Indexed(250),
            process_shadow: Color::Indexed(240),

            col_pid: Color::Green,
            col_user: Color::Indexed(250),
            col_priority: Color::Indexed(250),
            col_mem_high: Color::Green,
            col_mem_normal: Color::Indexed(250),
            col_cpu_high: Color::Red,
            col_cpu_medium: Color::Yellow,
            col_cpu_low: Color::Green,
            col_status_running: Color::Green,
            col_status_sleeping: Color::Indexed(250),
            col_status_disk_sleep: Color::Red,
            col_status_stopped: Color::Red,
            col_status_zombie: Color::Magenta,
            col_status_unknown: Color::Indexed(240),
            col_command: Color::Indexed(250),
            col_command_basename: Color::Green,
            col_thread: Color::Indexed(33),

            footer_key_fg: Color::Black,
            footer_key_bg: Color::Cyan,
            footer_label_fg: Color::Indexed(250),
            footer_label_bg: Color::Black,

            tab_active_bg: Color::Cyan,
            tab_active_fg: Color::Black,
            tab_inactive_fg: Color::Indexed(240),
            tab_inactive_bg: Color::Indexed(234),

            popup_border: Color::Cyan,
            popup_bg: Color::Black,
            popup_title: Color::Cyan,
            popup_selected_bg: Color::Cyan,
            popup_selected_fg: Color::Black,
            popup_text: Color::Indexed(250),

            search_label: Color::Cyan,
            search_text: Color::Indexed(250),
            filter_label: Color::Yellow,
            filter_text: Color::Indexed(250),
        }
    }

    /// Light Terminal — for white/light backgrounds
    fn light_terminal() -> Self {
        Self {
            bg: Color::White,

            cpu_bar_normal: Color::Green,
            cpu_bar_system: Color::Red,
            cpu_bar_low: Color::Blue,
            cpu_bar_virt: Color::Cyan,
            cpu_bar_iowait: Color::DarkGray,
            cpu_bar_irq: Color::Magenta,
            cpu_label: Color::Black,
            cpu_bar_bg: Color::Indexed(252),

            mem_bar_used: Color::Green,
            mem_bar_buffers: Color::Blue,
            mem_bar_cache: Color::Yellow,

            swap_bar: Color::Red,

            tasks_text: Color::Black,
            load_text: Color::Black,
            uptime_text: Color::Black,
            info_label: Color::Black,
            info_value: Color::Blue,

            table_header_bg: Color::Blue,
            table_header_fg: Color::White,
            table_header_sort_bg: Color::Green,
            table_header_sort_fg: Color::Black,

            process_fg: Color::Black,
            process_bg: Color::White,
            process_selected_bg: Color::Indexed(153),
            process_selected_fg: Color::Black,
            process_shadow: Color::Indexed(245),

            col_pid: Color::Blue,
            col_user: Color::Black,
            col_priority: Color::Black,
            col_mem_high: Color::Blue,
            col_mem_normal: Color::Black,
            col_cpu_high: Color::Red,
            col_cpu_medium: Color::Indexed(208),
            col_cpu_low: Color::Green,
            col_status_running: Color::Green,
            col_status_sleeping: Color::Black,
            col_status_disk_sleep: Color::Red,
            col_status_stopped: Color::Red,
            col_status_zombie: Color::Magenta,
            col_status_unknown: Color::Indexed(245),
            col_command: Color::Black,
            col_command_basename: Color::Blue,
            col_thread: Color::DarkGray,

            footer_key_fg: Color::White,
            footer_key_bg: Color::Blue,
            footer_label_fg: Color::Black,
            footer_label_bg: Color::White,

            tab_active_bg: Color::Blue,
            tab_active_fg: Color::White,
            tab_inactive_fg: Color::Indexed(245),
            tab_inactive_bg: Color::Indexed(254),

            popup_border: Color::Blue,
            popup_bg: Color::White,
            popup_title: Color::Blue,
            popup_selected_bg: Color::Blue,
            popup_selected_fg: Color::White,
            popup_text: Color::Black,

            search_label: Color::Blue,
            search_text: Color::Black,
            filter_label: Color::Indexed(208),
            filter_text: Color::Black,
        }
    }

    /// Midnight Commander style — blue background
    fn midnight_commander() -> Self {
        Self {
            bg: Color::Blue,

            cpu_bar_normal: Color::Green,
            cpu_bar_system: Color::Red,
            cpu_bar_low: Color::Cyan,
            cpu_bar_virt: Color::Yellow,
            cpu_bar_iowait: Color::DarkGray,
            cpu_bar_irq: Color::Magenta,
            cpu_label: Color::White,
            cpu_bar_bg: Color::Indexed(17),

            mem_bar_used: Color::Green,
            mem_bar_buffers: Color::Cyan,
            mem_bar_cache: Color::Yellow,

            swap_bar: Color::Red,

            tasks_text: Color::White,
            load_text: Color::White,
            uptime_text: Color::White,
            info_label: Color::White,
            info_value: Color::Yellow,

            table_header_bg: Color::Cyan,
            table_header_fg: Color::Blue,
            table_header_sort_bg: Color::Yellow,
            table_header_sort_fg: Color::Blue,

            process_fg: Color::White,
            process_bg: Color::Blue,
            process_selected_bg: Color::Cyan,
            process_selected_fg: Color::Blue,
            process_shadow: Color::Indexed(67),

            col_pid: Color::Yellow,
            col_user: Color::White,
            col_priority: Color::White,
            col_mem_high: Color::Yellow,
            col_mem_normal: Color::White,
            col_cpu_high: Color::Red,
            col_cpu_medium: Color::Yellow,
            col_cpu_low: Color::Green,
            col_status_running: Color::Green,
            col_status_sleeping: Color::White,
            col_status_disk_sleep: Color::Red,
            col_status_stopped: Color::Red,
            col_status_zombie: Color::Magenta,
            col_status_unknown: Color::Indexed(67),
            col_command: Color::White,
            col_command_basename: Color::Yellow,
            col_thread: Color::Cyan,

            footer_key_fg: Color::Blue,
            footer_key_bg: Color::Yellow,
            footer_label_fg: Color::White,
            footer_label_bg: Color::Blue,

            tab_active_bg: Color::Cyan,
            tab_active_fg: Color::Blue,
            tab_inactive_fg: Color::Indexed(67),
            tab_inactive_bg: Color::Blue,

            popup_border: Color::Yellow,
            popup_bg: Color::Blue,
            popup_title: Color::Yellow,
            popup_selected_bg: Color::Yellow,
            popup_selected_fg: Color::Blue,
            popup_text: Color::White,

            search_label: Color::Yellow,
            search_text: Color::White,
            filter_label: Color::Cyan,
            filter_text: Color::White,
        }
    }

    /// Black on White
    fn black_on_white() -> Self {
        Self {
            bg: Color::White,

            cpu_bar_normal: Color::Indexed(22),  // Dark green
            cpu_bar_system: Color::Indexed(124), // Dark red
            cpu_bar_low: Color::Indexed(25),     // Dark blue
            cpu_bar_virt: Color::Indexed(30),    // Dark cyan
            cpu_bar_iowait: Color::Indexed(245),
            cpu_bar_irq: Color::Indexed(127),
            cpu_label: Color::Black,
            cpu_bar_bg: Color::Indexed(252),

            mem_bar_used: Color::Indexed(22),
            mem_bar_buffers: Color::Indexed(25),
            mem_bar_cache: Color::Indexed(130),

            swap_bar: Color::Indexed(124),

            tasks_text: Color::Black,
            load_text: Color::Black,
            uptime_text: Color::Black,
            info_label: Color::Black,
            info_value: Color::Indexed(25),

            table_header_bg: Color::Indexed(250),
            table_header_fg: Color::Black,
            table_header_sort_bg: Color::Indexed(22),
            table_header_sort_fg: Color::White,

            process_fg: Color::Black,
            process_bg: Color::White,
            process_selected_bg: Color::Indexed(250),
            process_selected_fg: Color::Black,
            process_shadow: Color::Indexed(245),

            col_pid: Color::Indexed(25),
            col_user: Color::Black,
            col_priority: Color::Black,
            col_mem_high: Color::Indexed(25),
            col_mem_normal: Color::Black,
            col_cpu_high: Color::Indexed(124),
            col_cpu_medium: Color::Indexed(130),
            col_cpu_low: Color::Indexed(22),
            col_status_running: Color::Indexed(22),
            col_status_sleeping: Color::Black,
            col_status_disk_sleep: Color::Indexed(124),
            col_status_stopped: Color::Indexed(124),
            col_status_zombie: Color::Indexed(127),
            col_status_unknown: Color::Indexed(245),
            col_command: Color::Black,
            col_command_basename: Color::Indexed(25),
            col_thread: Color::Indexed(245),

            footer_key_fg: Color::White,
            footer_key_bg: Color::Indexed(25),
            footer_label_fg: Color::Black,
            footer_label_bg: Color::White,

            tab_active_bg: Color::Indexed(25),
            tab_active_fg: Color::White,
            tab_inactive_fg: Color::Indexed(245),
            tab_inactive_bg: Color::Indexed(254),

            popup_border: Color::Indexed(25),
            popup_bg: Color::White,
            popup_title: Color::Indexed(25),
            popup_selected_bg: Color::Indexed(25),
            popup_selected_fg: Color::White,
            popup_text: Color::Black,

            search_label: Color::Indexed(25),
            search_text: Color::Black,
            filter_label: Color::Indexed(130),
            filter_text: Color::Black,
        }
    }

    /// Dark Vivid — high contrast, vivid colors on dark background
    fn dark_vivid() -> Self {
        Self {
            bg: Color::Black,

            cpu_bar_normal: Color::Indexed(46),   // Bright green
            cpu_bar_system: Color::Indexed(196),   // Bright red
            cpu_bar_low: Color::Indexed(39),       // Bright blue
            cpu_bar_virt: Color::Indexed(51),      // Bright cyan
            cpu_bar_iowait: Color::Indexed(245),
            cpu_bar_irq: Color::Indexed(201),      // Bright magenta
            cpu_label: Color::White,
            cpu_bar_bg: Color::Indexed(235),

            mem_bar_used: Color::Indexed(46),
            mem_bar_buffers: Color::Indexed(39),
            mem_bar_cache: Color::Indexed(226),

            swap_bar: Color::Indexed(196),

            tasks_text: Color::White,
            load_text: Color::White,
            uptime_text: Color::White,
            info_label: Color::White,
            info_value: Color::Indexed(51),

            table_header_bg: Color::Indexed(236),
            table_header_fg: Color::Indexed(51),
            table_header_sort_bg: Color::Indexed(46),
            table_header_sort_fg: Color::Black,

            process_fg: Color::Indexed(252),
            process_bg: Color::Black,
            process_selected_bg: Color::Indexed(236),
            process_selected_fg: Color::Indexed(226),
            process_shadow: Color::Indexed(240),

            col_pid: Color::Indexed(46),
            col_user: Color::Indexed(252),
            col_priority: Color::Indexed(252),
            col_mem_high: Color::Indexed(46),
            col_mem_normal: Color::Indexed(252),
            col_cpu_high: Color::Indexed(196),
            col_cpu_medium: Color::Indexed(226),
            col_cpu_low: Color::Indexed(46),
            col_status_running: Color::Indexed(46),
            col_status_sleeping: Color::Indexed(252),
            col_status_disk_sleep: Color::Indexed(196),
            col_status_stopped: Color::Indexed(196),
            col_status_zombie: Color::Indexed(201),
            col_status_unknown: Color::Indexed(240),
            col_command: Color::Indexed(252),
            col_command_basename: Color::Indexed(46),
            col_thread: Color::Indexed(39),

            footer_key_fg: Color::Black,
            footer_key_bg: Color::Indexed(51),
            footer_label_fg: Color::Indexed(252),
            footer_label_bg: Color::Black,

            tab_active_bg: Color::Indexed(51),
            tab_active_fg: Color::Black,
            tab_inactive_fg: Color::Indexed(240),
            tab_inactive_bg: Color::Indexed(233),

            popup_border: Color::Indexed(51),
            popup_bg: Color::Black,
            popup_title: Color::Indexed(51),
            popup_selected_bg: Color::Indexed(51),
            popup_selected_fg: Color::Black,
            popup_text: Color::Indexed(252),

            search_label: Color::Indexed(51),
            search_text: Color::Indexed(252),
            filter_label: Color::Indexed(226),
            filter_text: Color::Indexed(252),
        }
    }

    // ── Convenience style builders ──────────────────────────────────────

    pub fn header_normal_style(&self) -> Style {
        Style::default().fg(self.cpu_bar_normal)
    }

    pub fn header_system_style(&self) -> Style {
        Style::default().fg(self.cpu_bar_system)
    }

    pub fn table_header_style(&self) -> Style {
        Style::default().fg(self.table_header_fg).bg(self.table_header_bg)
    }

    pub fn table_header_sort_style(&self) -> Style {
        Style::default()
            .fg(self.table_header_sort_fg)
            .bg(self.table_header_sort_bg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn process_style(&self) -> Style {
        Style::default().fg(self.process_fg).bg(self.process_bg)
    }

    pub fn selected_style(&self) -> Style {
        Style::default().fg(self.process_selected_fg).bg(self.process_selected_bg)
    }

    pub fn footer_key_style(&self) -> Style {
        Style::default().fg(self.footer_key_fg).bg(self.footer_key_bg)
    }

    pub fn footer_label_style(&self) -> Style {
        Style::default().fg(self.footer_label_fg).bg(self.footer_label_bg)
    }
}
