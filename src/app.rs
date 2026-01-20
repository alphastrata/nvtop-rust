use nvml_wrapper::enum_wrappers::device::{Clock, ClockId};
use nvml_wrapper::error::NvmlError;
use nvml_wrapper::struct_wrappers::device::PciInfo;
use nvml_wrapper::{enum_wrappers::device::TemperatureSensor, struct_wrappers::device::MemoryInfo};

use ratatui::symbols::DOT;
use ratatui::{prelude::*, widgets::*};
use ratatui::{
    prelude::{CrosstermBackend, Terminal},
    widgets::Block,
    widgets::Paragraph,
};

use std::time::Duration;

use crate::errors::NvTopError;
use crate::stylers::calculate_severity;
use crate::termite::LoggingHandle;
use crate::{errors, gpu::GpuInfo};

#[derive(Clone, PartialEq)]
enum ProcessSortBy {
    Memory,
    Name,
    Pid,
}

#[derive(Debug)]
struct GpuProcess {
    pid: u32,
    used_memory: u64,
    name: String,
}

pub fn run(
    nvml: nvml_wrapper::Nvml,
    delay: Duration,
    lh: &LoggingHandle,
) -> anyhow::Result<(), errors::NvTopError> {
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(std::io::stderr(), crossterm::terminal::EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stderr()))?;
    lh.debug("crossterm initialisation successful");

    let mut gpu_list = crate::gpu::try_init_gpus(&nvml, lh)?;

    let mut selected_gpu: usize = 0;
    let mut have_fans: bool = gpu_list
        .iter()
        .any(|gpu| gpu.inner.num_fans().map_or(0, |fc| fc) != 0);

    // State variables for process view
    let mut show_process_view: bool = true;
    let mut fuzzy_search_active: bool = false;
    let mut fuzzy_search_input: String = String::new();

    let mut sort_by: ProcessSortBy = ProcessSortBy::Memory;
    let mut sort_reverse: bool = true;

    // State for process selection
    let mut selected_process_pid: Option<u32> = None;
    let mut process_selection_enabled: bool = false;
    let mut highlighted_process_index: usize = 0;

    lh.debug(&format!("GPU has fans = {}", have_fans));

    loop {
        _ = terminal.draw(|f| {
            let gpu = &gpu_list[selected_gpu];

            // draw tab bar if more than one device is connected
            let mid_area = if gpu_list.len() == 1 {
                let layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(1)])
                    .split(f.area());

                #[cfg(target_os = "linux")]
                f.render_widget(
                    if show_process_view {
                        Paragraph::new("q to quit, p to rescan devices, ESC to return to stats").alignment(Alignment::Left)
                    } else {
                        Paragraph::new("q to quit, p to rescan devices").alignment(Alignment::Right)
                    },
                    layout[1],
                );

                #[cfg(target_os = "windows")]
                f.render_widget(
                    Paragraph::new(if show_process_view {
                        "q to quit, ESC to return to stats"
                    } else {
                        "q to quit"
                    }),
                    layout[1],
                );

                layout[0]
            } else {
                let layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(1),
                        Constraint::Min(0),
                        Constraint::Length(1),
                    ])
                    .split(f.area());

                f.render_widget(
                    Tabs::new(
                        gpu_list
                            .iter()
                            .map(|gpu| format!("[{}] {}", gpu.index, gpu.card_type))
                            .collect::<Vec<_>>(),
                    )
                    .select(selected_gpu)
                    .style(Style::default().fg(Color::Green))
                    .highlight_style(Style::default().fg(Color::Green).bold())
                    .divider(DOT),
                    layout[0],
                );

                // Render the main footer text
                f.render_widget(
                    if show_process_view {
                        if process_selection_enabled {
                            Paragraph::new("q to quit, ESC to return to stats, f or / to search, s to sort, r to reverse, UP/DOWN to navigate, SPACE to select, p to exit".to_string())
                        } else {
                            Paragraph::new("q to quit, p to rescan devices, fn keys to switch devices, ESC to return to stats, f or / to search, s to sort, r to reverse, p to select process".to_string())
                        }
                    } else {
                        Paragraph::new("q to quit, p to rescan devices, fn keys to switch devices, f or / to show processes".to_string())
                    },
                    layout[2],
                );

                layout[1]
            };

            // Outermost Block, which draws the green border around the whole UI.
            let block = Block::default()
                .title("NVTOP")
                .title_top(Line::from("NVTOP"))
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green))
                .border_type(BorderType::Rounded)
                .style(Style::default());
            f.render_widget(block, mid_area);

            let main_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Percentage(50),
                    Constraint::Percentage(45),
                ])
                .margin(1)
                .split(f.area());

            // Top section: Card info (single line)
            let top_chunk = main_layout[0];
            let card_info_widget = draw_driver_info(gpu);
            f.render_widget(card_info_widget, top_chunk);

            // Middle section: Metrics (core util, temp, memory, etc.)
            let middle_area = main_layout[1];
            let middle_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(70),
                    Constraint::Percentage(30),
                ])
                .split(middle_area);

            let left_middle = Layout::default()
                .constraints([
                    Constraint::Percentage(50), // Core utilisation
                    Constraint::Percentage(50), // Core Clock
                ])
                .split(middle_chunks[0]);

            // Core utilisation
            let core_gauge = draw_core_utilisation(gpu);
            f.render_widget(core_gauge, left_middle[0]);

            // Core Clock
            let core_gauge = draw_core_clock(gpu).unwrap();
            f.render_widget(core_gauge, left_middle[1]);

            // Right side of middle: Memory, Temp, Fan - adjusted to align with left panels
            let right_middle = Layout::default()
                .constraints([
                    Constraint::Percentage(50), // Memory
                    Constraint::Percentage(25), // Temp (half height)
                    Constraint::Percentage(25), // Fan speed (half height)
                ])
                .split(middle_chunks[1]);

            // Memory:
            let mem_usage_gauge = draw_memory_usage(gpu);
            f.render_widget(mem_usage_gauge, right_middle[0]);

            // Temp:
            let temp_gauge = draw_gpu_die_temp(gpu);
            f.render_widget(temp_gauge, right_middle[1]);

            // Fan speed:
            if have_fans {
                let gauge = draw_fan_speed(gpu);
                f.render_widget(gauge, right_middle[2]);
            }

            // Bottom section: Processes (always shown)
            let process_widget = draw_misc_with_processes(gpu, &fuzzy_search_input, &sort_by, sort_reverse, process_selection_enabled, highlighted_process_index, selected_process_pid);
            f.render_widget(process_widget, main_layout[2]);

            // If fuzzy search is active, render a modal-style search box in the Center as an overlay
            if fuzzy_search_active {
                let search_text = format!("{}_", &fuzzy_search_input);
                let block = Block::default()
                    .borders(Borders::ALL)
                    .title("🔍... [ESC to cancel]")
                    .border_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

                let search_input = Paragraph::new(search_text)
                    .block(block)
                    .style(Style::default().bg(Color::Rgb(40, 40, 40)).fg(Color::LightCyan).add_modifier(Modifier::BOLD))
                    .alignment(Alignment::Left);

                // Centre the search box
                let search_width = 60.min(f.area().width.saturating_sub(2));
                let search_height = 3;
                let x = f.area().width / 2 - search_width / 2;
                let y = f.area().height / 2 - 1; // Vertically..

                let search_area = Rect::new(x, y, search_width, search_height);

                // Draw a semi-transparent overlay background
                let overlay = Paragraph::new(" ")
                    .style(Style::default().bg(Color::Rgb(20, 20, 20))); // Very dark semi-transparent?

                f.render_widget(overlay, f.area());
                f.render_widget(search_input, search_area);
            }
        })?;

        if crossterm::event::poll(std::time::Duration::from_millis(250))?
            && let crossterm::event::Event::Key(key) = crossterm::event::read()?
        {
            use crossterm::event::KeyCode;

            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Esc => {
                    // Exit fuzzy search if active, otherwise exit process view mode if currently in it
                    if fuzzy_search_active {
                        fuzzy_search_active = false;
                        fuzzy_search_input.clear();
                    } else if show_process_view {
                        show_process_view = false;
                    }
                }
                KeyCode::Char('f') | KeyCode::Char('/') => {
                    // Always activate fuzzy search when '/' is pressed
                    show_process_view = true;
                    fuzzy_search_active = true;
                    fuzzy_search_input.clear();
                }
                // Handle character input for fuzzy search - but exclude 'f' and '/' which are handled separately
                KeyCode::Char(c) if fuzzy_search_active => {
                    fuzzy_search_input.push(c);
                }
                // Handle backspace in fuzzy search
                KeyCode::Backspace if fuzzy_search_active => {
                    fuzzy_search_input.pop();
                }
                // Handle Enter to exit fuzzy search
                KeyCode::Enter if fuzzy_search_active => {
                    fuzzy_search_active = false;
                }
                // Sorting controls
                KeyCode::Char('s') => {
                    // Cycle through sorting options: Memory -> Name -> PID -> Memory...
                    sort_by = match sort_by {
                        ProcessSortBy::Memory => ProcessSortBy::Name,
                        ProcessSortBy::Name => ProcessSortBy::Pid,
                        ProcessSortBy::Pid => ProcessSortBy::Memory,
                    };
                }
                KeyCode::Char('r') => {
                    // Reverse sort order
                    sort_reverse = !sort_reverse;
                }
                // Process selection controls
                KeyCode::Char('p') if show_process_view && !fuzzy_search_active => {
                    // Toggle process selection mode
                    process_selection_enabled = !process_selection_enabled;
                    if !process_selection_enabled {
                        selected_process_pid = None;
                    }
                }
                KeyCode::Char(' ') if process_selection_enabled && show_process_view => {
                    // Select the currently highlighted process
                    if let Ok(processes) = get_gpu_processes(&gpu_list[selected_gpu])
                        && highlighted_process_index < processes.len()
                        && let Some(proc) = processes.get(highlighted_process_index)
                    {
                        selected_process_pid = Some(proc.pid);
                        process_selection_enabled = false;
                    }
                }
                KeyCode::Up if process_selection_enabled && show_process_view => {
                    // Move highlight up
                    highlighted_process_index = highlighted_process_index.saturating_sub(1);
                }
                KeyCode::Down if process_selection_enabled && show_process_view => {
                    // Move highlight down
                    if let Ok(processes) = get_gpu_processes(&gpu_list[selected_gpu])
                        && highlighted_process_index < processes.len().saturating_sub(1)
                    {
                        highlighted_process_index += 1;
                    }
                }
               
                #[cfg(target_os = "linux")]
                KeyCode::Char('m') => {
                    // re-scan pci tree to let driver discover new devices (only works as sudo)
                    match nvml.discover_gpus(PciInfo {
                        bus: 0,
                        bus_id: "".into(),
                        device: 0,
                        domain: 0,
                        pci_device_id: 0,
                        pci_sub_system_id: Some(0),
                    }) {
                        Ok(()) => {
                            have_fans = gpu_list
                                .iter()
                                .any(|gpu| gpu.inner.num_fans().map_or(0, |fc| fc) != 0);

                            lh.debug(&format!("GPU has fans = {}", have_fans));
                            lh.debug("Re-scanned PCI tree");
                        }
                        Err(e @ (NvmlError::OperatingSystem | NvmlError::NoPermission)) => {
                            lh.debug(&format!("Failed to re-scan PCI tree: {e}"));
                        }
                        Err(e) => return Err(e.into()),
                    }
                    // re-scan for devices
                    gpu_list = crate::gpu::try_init_gpus(&nvml, lh)?;
                    if selected_gpu >= gpu_list.len() {
                        selected_gpu = 0;
                    }
                }
                _ => {}
            }
        }

        // primitive rate limiting.
        std::thread::sleep(delay);
    }

    crossterm::execute!(std::io::stderr(), crossterm::terminal::LeaveAlternateScreen)?;
    crossterm::terminal::disable_raw_mode()?;

    Ok(())
}

fn draw_fan_speed<'d>(gpu: &GpuInfo<'d>) -> Gauge<'d> {
    let temps = gpu
        .inner
        .num_fans()
        .expect("This should be impossible as we never call this without having earlier checked.");
    let avg = (0..temps as usize)
        .flat_map(|v| gpu.inner.fan_speed(v as u32))
        .map(|u| u as f64)
        .sum::<f64>()
        / temps as f64;

    let percentage = (avg / 100.).clamp(0.0, 1.0);

    let label = format!("{:.1}%", avg);
    let spanned_label = Span::styled(label, Style::new().white().bold().bg(Color::Black));

    Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Fan Speed"))
        .gauge_style(calculate_severity(percentage).style_for())
        .label(spanned_label)
        .style(Style::default())
        .ratio(percentage)
}

fn draw_gpu_die_temp<'d>(gpu: &GpuInfo<'d>) -> Gauge<'d> {
    let gpu_die_temperature = gpu
        .inner
        .temperature(TemperatureSensor::Gpu)
        .map_or(0, |temp| temp);

    let label = format!("{:.2}°C", gpu_die_temperature);
    let spanned_label = Span::styled(label, Style::new().white().bold().bg(Color::Black));
    let temp_ratio = (gpu_die_temperature as f64 / 100.).clamp(0.0, 1.0);

    Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Temp"))
        .gauge_style(calculate_severity(temp_ratio).style_for())
        .label(spanned_label)
        .style(Style::default())
        .ratio(temp_ratio)
}

fn draw_memory_usage<'d>(gpu: &GpuInfo<'d>) -> Gauge<'d> {
    let mem_info = gpu.inner.memory_info().map_or(
        MemoryInfo {
            free: 0,
            total: 0, //TODO: This never changes so put in self
            used: 0,
            reserved: Default::default(),
            version: Default::default(),
        },
        |mem_info| mem_info,
    );

    let mem_used = mem_info.used as f64 / 1_073_741_824.0; // as GB
    let mem_total = mem_info.total as f64 / 1_073_741_824.0;
    let mem_percentage = (mem_used / mem_total).clamp(0.0, 1.0);

    let label = format!("{:.2}/{:.2}GB", mem_used, mem_total);
    let spanned_label = Span::styled(label, Style::new().white().bold().bg(Color::Black));

    Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Memory Usage"))
        .gauge_style(calculate_severity(mem_percentage).style_for())
        .label(spanned_label)
        .ratio(mem_percentage)
}

fn draw_driver_info<'d>(gpu: &GpuInfo<'d>) -> Paragraph<'d> {
    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        "Card Info",
        Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::BOLD),
    ));

    let compute_cap = match gpu.inner.cuda_compute_capability() {
        Ok(cap) => format!("{}.{}", cap.major, cap.minor),
        Err(_) => "N/A".to_string(),
    };

    let info_text = format!(
        "Card: {:<15} | Driver: {:<12} | CUDA: {:<8} | Compute Cap: {}",
        gpu.card_type,
        gpu.driver_version,
        gpu.cuda_version / 1000.0,
        compute_cap
    );

    Paragraph::new(info_text)
        .block(block)
        .wrap(Wrap { trim: true })
}

fn draw_misc_with_processes<'d>(
    gpu: &GpuInfo<'d>,
    search_term: &str,
    sort_by: &ProcessSortBy,
    sort_reverse: bool,
    process_selection_enabled: bool,
    highlighted_process_index: usize,
    selected_process_pid: Option<u32>,
) -> Paragraph<'d> {
    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        "GPU Processes",
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD),
    ));

    let content = match get_gpu_processes(gpu) {
        Ok(mut processes) => {
            if processes.is_empty() {
                format!("{}\n\nNo processes running", gpu.misc)
            } else {
                // Filter and rank processes based on search term if provided
                if !search_term.is_empty() {
                    // Create pairs of (process, score) and filter by positive scores
                    let mut scored_processes: Vec<(GpuProcess, i32)> = Vec::new();

                    for proc in processes {
                        let name_score = fuzzy_score(&proc.name, search_term);
                        let pid_score = fuzzy_score(&proc.pid.to_string(), search_term);

                        // Use the higher of the two scores
                        let max_score = name_score.max(pid_score);

                        if max_score > 0 {
                            scored_processes.push((proc, max_score));
                        }
                    }

                    // Sort by score (descending) - highest scores first
                    scored_processes.sort_by(|a, b| b.1.cmp(&a.1));

                    // Extract just the processes in ranked order
                    processes = scored_processes.into_iter().map(|(proc, _)| proc).collect();
                } else {
                    // Sort by selected column when not searching
                    match sort_by {
                        ProcessSortBy::Memory => {
                            if sort_reverse {
                                processes.sort_by(|a, b| b.used_memory.cmp(&a.used_memory));
                            // Descending
                            } else {
                                processes.sort_by(|a, b| a.used_memory.cmp(&b.used_memory));
                                // Ascending
                            }
                        }
                        ProcessSortBy::Name => {
                            if sort_reverse {
                                processes.sort_by(|a, b| {
                                    b.name.to_lowercase().cmp(&a.name.to_lowercase())
                                }); // Descending
                            } else {
                                processes.sort_by(|a, b| {
                                    a.name.to_lowercase().cmp(&b.name.to_lowercase())
                                }); // Ascending
                            }
                        }
                        ProcessSortBy::Pid => {
                            if sort_reverse {
                                processes.sort_by(|a, b| b.pid.cmp(&a.pid)); // Descending
                            } else {
                                processes.sort_by(|a, b| a.pid.cmp(&b.pid)); // Ascending
                            }
                        }
                    }
                }

                if processes.is_empty() && !search_term.is_empty() {
                    format!("No processes match '{}'", search_term)
                } else {
                    let mut content = String::new();
                    //TODO: Nicer stringing without these manual spaces...
                    content.push_str("Name                                    PID              Memory(MB)  Type\n");
                    for (idx, proc) in processes.iter().take(40).enumerate() {
                        let memory_mb = proc.used_memory / 1024 / 1024;

                        // Determine process type (this is a simplified approach)
                        let process_type = if proc.name.contains("comp")
                            || proc.name.contains("cuda")
                            || proc.name.contains("nvidia")
                        {
                            "C" // Compute
                        } else {
                            "G" // Graphics
                        };

                        // Row prefix for display
                        let row_prefix = "  "; // Using spaces for appearance

                        // Highlight the currently selected/highlighted process
                        //BUG: this doesn't work...
                        let line = if process_selection_enabled && idx == highlighted_process_index
                        {
                            format!(
                                "> {:<38} {:>15} {:>13} MB  {}\n",
                                proc.name, proc.pid, memory_mb, process_type
                            )
                        } else if selected_process_pid == Some(proc.pid) {
                            format!(
                                "* {:<38} {:>15} {:>13} MB  {}\n",
                                proc.name, proc.pid, memory_mb, process_type
                            )
                        } else {
                            format!(
                                "{}{:<38} {:>15} {:>13} MB  {}\n",
                                row_prefix, proc.name, proc.pid, memory_mb, process_type
                            )
                        };

                        content.push_str(&line);
                    }
                    content
                }
            }
        }
        Err(_) => {
            // Fallback: show misc info with error message
            format!("{}\n\nError retrieving processes", gpu.misc)
        }
    };

    Paragraph::new(content)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((0, 0))
}

fn draw_core_utilisation<'d>(gpu: &GpuInfo<'d>) -> Gauge<'d> {
    let utilisation_rates = gpu.inner.utilization_rates();
    let percent = utilisation_rates.map_or(0, |ur| ur.gpu as u16);

    let spanned_label = Span::styled(
        format!("{}%", percent),
        Style::new().white().bold().bg(Color::Black),
    );

    Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Core Utilisation"),
        )
        .gauge_style(Style {
            fg: Some(Color::Green),
            bg: None,
            underline_color: None,
            add_modifier: Modifier::BOLD,
            sub_modifier: Modifier::BOLD,
        })
        .percent(percent)
        .label(spanned_label)
}

fn draw_core_clock<'d>(gpu: &GpuInfo<'d>) -> Result<Gauge<'d>, NvTopError> {
    let current_clock = gpu.inner.clock(Clock::Graphics, ClockId::Current)?;
    let percentage = (current_clock as f64 / gpu.max_core_clock as f64).clamp(0.0, 1.0);

    let label = format!("{}/{}Mhz", current_clock, gpu.max_core_clock);
    let spanned_label = Span::styled(label, Style::new().white().bold().bg(Color::Black));

    Ok(Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Core Clock"))
        .gauge_style(calculate_severity(percentage).style_for())
        .label(spanned_label)
        .ratio(percentage))
}

// Helper function for fuzzy matching using a more sophisticated algorithm
fn fuzzy_match(text: &str, pattern: &str) -> bool {
    if pattern.is_empty() {
        return true;
    }

    let text_chars: Vec<char> = text.chars().collect();
    let pattern_chars: Vec<char> = pattern.chars().collect();

    let mut text_idx = 0;
    let mut pattern_idx = 0;

    while text_idx < text_chars.len() && pattern_idx < pattern_chars.len() {
        if text_chars[text_idx]
            .to_lowercase()
            .eq(pattern_chars[pattern_idx].to_lowercase())
        {
            pattern_idx += 1;
        }
        text_idx += 1;
    }

    pattern_idx == pattern_chars.len()
}

// Helper to score fuzzy matches (higher score = better match)
fn fuzzy_score(text: &str, pattern: &str) -> i32 {
    if pattern.is_empty() {
        return 0;
    }

    let text_lower = text.to_lowercase();
    let pattern_lower = pattern.to_lowercase();

    if !fuzzy_match(text, pattern) {
        return -1; // No match
    }

    // Calculate match quality
    let mut score = 0;
    let text_chars: Vec<char> = text_lower.chars().collect();
    let pattern_chars: Vec<char> = pattern_lower.chars().collect();

    let mut text_idx = 0;
    let mut pattern_idx = 0;
    let mut consecutive_bonus = 0;

    // Simple scoring algorithm: find matches and give bonuses for consecutive letters
    while text_idx < text_chars.len() && pattern_idx < pattern_chars.len() {
        if text_chars[text_idx] == pattern_chars[pattern_idx] {
            // Bonus for consecutive matches
            if text_idx > 0
                && pattern_idx > 0
                && text_chars[text_idx - 1] == pattern_chars[pattern_idx - 1]
            {
                consecutive_bonus += 10;
            } else {
                consecutive_bonus = 10; // Base bonus for a match
            }

            score += consecutive_bonus + 5; // Base score plus consecutive bonus
            pattern_idx += 1;
        } else {
            consecutive_bonus = 0; // Reset consecutive bonus
        }

        text_idx += 1;
    }

    score
}

fn get_gpu_processes<'d>(gpu: &GpuInfo<'d>) -> Result<Vec<GpuProcess>, NvmlError> {
    let mut gpu_processes = Vec::new();

    if let Ok(compute_processes) = gpu.inner.running_compute_processes() {
        for process in compute_processes {
            let name = match std::fs::read_to_string(format!("/proc/{}/comm", process.pid)) {
                Ok(name) => name.trim().to_string(),
                Err(_) => format!("Process-{}", process.pid),
            };

            let memory_value = extract_memory_value(process.used_gpu_memory);

            gpu_processes.push(GpuProcess {
                pid: process.pid,
                used_memory: memory_value,
                name,
            });
        }
    }

    if let Ok(graphics_processes) = gpu.inner.running_graphics_processes() {
        for process in graphics_processes {
            if !gpu_processes.iter().any(|p| p.pid == process.pid) {
                let name = match std::fs::read_to_string(format!("/proc/{}/comm", process.pid)) {
                    Ok(name) => name.trim().to_string(),
                    Err(_) => format!("Process-{}", process.pid),
                };

                let memory_value = extract_memory_value(process.used_gpu_memory);

                gpu_processes.push(GpuProcess {
                    pid: process.pid,
                    used_memory: memory_value,
                    name,
                });
            }
        }
    }

    Ok(gpu_processes)
}

fn extract_memory_value(used_memory: nvml_wrapper::enums::device::UsedGpuMemory) -> u64 {
    let s = format!("{:?}", used_memory);

    if let Some(start) = s.find('(')
        && let Some(end) = s.find(')')
        && start < end
    {
        let num_str = &s[start + 1..end];
        return num_str.parse::<u64>().unwrap_or(0);
    }
    s.chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse::<u64>()
        .unwrap_or(0)
}
