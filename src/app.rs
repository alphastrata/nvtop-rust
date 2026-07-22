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
use crate::{
    errors,
    gpu::{GpuInfo, GpuProcess, get_gpu_processes},
};
use crossterm::event::KeyCode;

#[derive(Clone, PartialEq)]
enum ProcessSortBy {
    Memory,
    Name,
    Pid,
}

/// Which tab is active: the Master overview or a single GPU.
#[derive(Clone, Copy, PartialEq)]
enum ActiveTab {
    Master,
    Gpu(usize),
}

impl ActiveTab {
    fn next(self, count: usize) -> Self {
        match self {
            ActiveTab::Master => {
                if count > 0 {
                    ActiveTab::Gpu(0)
                } else {
                    ActiveTab::Master
                }
            }
            ActiveTab::Gpu(i) => {
                if i + 1 < count {
                    ActiveTab::Gpu(i + 1)
                } else {
                    ActiveTab::Master
                }
            }
        }
    }

    fn prev(self, count: usize) -> Self {
        match self {
            ActiveTab::Master => {
                if count > 0 {
                    ActiveTab::Gpu(count - 1)
                } else {
                    ActiveTab::Master
                }
            }
            ActiveTab::Gpu(i) => {
                if i > 0 {
                    ActiveTab::Gpu(i - 1)
                } else {
                    ActiveTab::Master
                }
            }
        }
    }

    fn index(self) -> usize {
        match self {
            ActiveTab::Master => 0,
            ActiveTab::Gpu(i) => i + 1,
        }
    }
}

pub fn run(
    nvml: nvml_wrapper::Nvml,
    delay: Duration,
    lh: &LoggingHandle,
    hook_pid: Option<u32>,
) -> anyhow::Result<(), errors::NvTopError> {
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(std::io::stderr(), crossterm::terminal::EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stderr()))?;
    lh.debug("crossterm initialisation successful");

    let mut gpu_list = crate::gpu::try_init_gpus(&nvml, lh)?;

    let mut active_tab: ActiveTab = ActiveTab::Gpu(0);

    let mut have_fans: bool = gpu_list
        .iter()
        .any(|gpu| gpu.inner.num_fans().unwrap_or(0) != 0);

    // State variables for process view
    let mut show_process_view: bool = true;
    let mut fuzzy_search_active: bool = false;
    let mut fuzzy_search_input: String = String::new();

    let mut sort_by: ProcessSortBy = ProcessSortBy::Memory;
    let mut sort_reverse: bool = true;

    // State for process selection
    let mut selected_process_pid: Option<u32> = hook_pid;
    let mut process_selection_enabled: bool = hook_pid.is_some();
    let mut highlighted_process_index: usize = 0;

    lh.debug(&format!("GPU has fans = {}", have_fans));

    loop {
        _ = terminal.draw(|f| {
            let multi = gpu_list.len() > 1;

            let mid_area = if !multi {
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

                // Tab bar: Master + per-GPU tabs
                let tab_titles: Vec<String> = std::iter::once("Master".to_string())
                    .chain(gpu_list.iter().map(|gpu| format!("[{}] {}", gpu.index, gpu.name)))
                    .collect();

                f.render_widget(
                    Tabs::new(tab_titles)
                        .select(active_tab.index())
                        .style(Style::default().fg(Color::Green))
                        .highlight_style(Style::default().fg(Color::Cyan).bold().underlined())
                        .divider(DOT),
                    layout[0],
                );

                // Footer
                let footer = match active_tab {
                    ActiveTab::Master => {
                        Paragraph::new("q to quit, ←/→ or Tab to switch tabs, p to rescan devices")
                    }
                    ActiveTab::Gpu(_) => {
                        if show_process_view {
                            if process_selection_enabled {
                                Paragraph::new("q to quit, ←/→ or Tab to switch GPUs, ESC to return to stats, f or / to search, s to sort, r to reverse, ↑/↓ to navigate, SPACE to select, p to exit")
                            } else {
                                Paragraph::new("q to quit, ←/→ or Tab to switch GPUs, ESC to return to stats, f or / to search, s to sort, r to reverse, p to select process")
                            }
                        } else {
                            Paragraph::new("q to quit, ←/→ or Tab to switch GPUs, f or / to show processes")
                        }
                    }

                };
                f.render_widget(footer, layout[2]);

                layout[1]
            };

            match active_tab {
                ActiveTab::Master if multi => {
                    draw_master_view(f, mid_area, &gpu_list, have_fans);
                }
                ActiveTab::Master => {
                    // Single GPU: Master == Gpu(0), just render normally
                    draw_single_gpu_view(f, mid_area, &gpu_list[0], have_fans,
                        &fuzzy_search_input, &sort_by, sort_reverse,
                        process_selection_enabled, highlighted_process_index,

                        selected_process_pid, show_process_view);
                }
                ActiveTab::Gpu(idx) => {
                    if idx < gpu_list.len() {
                        draw_single_gpu_view(f, mid_area, &gpu_list[idx], have_fans,
                            &fuzzy_search_input, &sort_by, sort_reverse,
                            process_selection_enabled, highlighted_process_index,
                            selected_process_pid, show_process_view);
                    }
                }
            }

            // Fuzzy search overlay
            if fuzzy_search_active {
                let search_text = format!("{}_", fuzzy_search_input);
                let block = Block::default()
                    .borders(Borders::ALL)
                    .title("🔍... [ESC to cancel]")
                    .border_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

                let search_input = Paragraph::new(search_text)
                    .block(block)
                    .style(Style::default().bg(Color::Rgb(40, 40, 40)).fg(Color::LightCyan).add_modifier(Modifier::BOLD))
                    .alignment(Alignment::Left);

                let search_width = 60.min(f.area().width.saturating_sub(2));
                let search_height = 3;
                let x = f.area().width / 2 - search_width / 2;
                let y = f.area().height / 2 - 1;

                let search_area = Rect::new(x, y, search_width, search_height);

                f.render_widget(Clear, search_area);
                f.render_widget(search_input, search_area);
            }
        })?;

        if crossterm::event::poll(std::time::Duration::from_millis(250))?
            && let crossterm::event::Event::Key(key) = crossterm::event::read()?
            && key.kind == crossterm::event::KeyEventKind::Press
        {
            // Tab navigation (works in all views, but only meaningful when multi-GPU)
            if gpu_list.len() > 1 {
                match key.code {
                    KeyCode::Tab => {
                        active_tab = active_tab.next(gpu_list.len());
                        continue;
                    }
                    KeyCode::Right => {
                        if !fuzzy_search_active {
                            active_tab = active_tab.next(gpu_list.len());

                            continue;
                        }
                    }
                    KeyCode::Left
                        if !fuzzy_search_active => {
                            active_tab = active_tab.prev(gpu_list.len());
                            continue;
                        }
                    _ => {}
                }
            }

            // In Master view, only handle q and p
            if active_tab == ActiveTab::Master && gpu_list.len() > 1 {
                match key.code {
                    KeyCode::Char('q') => break,
                    #[cfg(target_os = "linux")]
                    KeyCode::Char('m') => {
                        rescan_gpus(&nvml, &mut gpu_list, &mut active_tab, &mut have_fans, lh)?;
                    }
                    _ => {}
                }
                std::thread::sleep(delay);
                continue;
            }

            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Esc => {
                    if fuzzy_search_active {
                        fuzzy_search_active = false;
                        fuzzy_search_input.clear();
                    } else if show_process_view {
                        show_process_view = false;
                    }
                }
                KeyCode::Char('f' | '/') if !fuzzy_search_active => {
                    show_process_view = true;
                    fuzzy_search_active = true;

                    fuzzy_search_input.clear();
                }
                KeyCode::Char(c) if fuzzy_search_active => {
                    fuzzy_search_input.push(c);
                }
                KeyCode::Backspace if fuzzy_search_active => {
                    fuzzy_search_input.pop();
                }
                KeyCode::Enter if fuzzy_search_active => {
                    fuzzy_search_active = false;
                }
                KeyCode::Char('s') => {
                    sort_by = match sort_by {
                        ProcessSortBy::Memory => ProcessSortBy::Name,
                        ProcessSortBy::Name => ProcessSortBy::Pid,
                        ProcessSortBy::Pid => ProcessSortBy::Memory,
                    };
                }
                KeyCode::Char('r') => {
                    sort_reverse = !sort_reverse;
                }
                KeyCode::Char('p') if show_process_view && !fuzzy_search_active => {
                    process_selection_enabled = !process_selection_enabled;
                    if !process_selection_enabled {
                        selected_process_pid = None;
                    }
                }
                KeyCode::Char(' ') if process_selection_enabled && show_process_view => {
                    if let ActiveTab::Gpu(idx) = active_tab
                        && idx < gpu_list.len()
                        && let Ok(processes) = get_gpu_processes(&gpu_list[idx])
                        && highlighted_process_index < processes.len()
                        && let Some(proc) = processes.get(highlighted_process_index)
                    {
                        selected_process_pid = Some(proc.pid);
                        process_selection_enabled = false;
                    }
                }
                KeyCode::Up if process_selection_enabled && show_process_view => {
                    highlighted_process_index = highlighted_process_index.saturating_sub(1);
                }
                KeyCode::Down if process_selection_enabled && show_process_view => {
                    if let ActiveTab::Gpu(idx) = active_tab
                        && idx < gpu_list.len()
                        && let Ok(processes) = get_gpu_processes(&gpu_list[idx])
                        && highlighted_process_index < processes.len().saturating_sub(1)
                    {
                        highlighted_process_index += 1;
                    }
                }
                #[cfg(target_os = "linux")]
                KeyCode::Char('m') => {
                    rescan_gpus(&nvml, &mut gpu_list, &mut active_tab, &mut have_fans, lh)?;
                }
                _ => {}
            }
        }

        std::thread::sleep(delay);
    }

    crossterm::execute!(std::io::stderr(), crossterm::terminal::LeaveAlternateScreen)?;
    crossterm::terminal::disable_raw_mode()?;

    Ok(())
}

#[cfg(target_os = "linux")]
fn rescan_gpus<'a>(
    nvml: &'a nvml_wrapper::Nvml,
    gpu_list: &mut Vec<GpuInfo<'a>>,
    active_tab: &mut ActiveTab,
    have_fans: &mut bool,
    lh: &LoggingHandle,
) -> Result<(), NvTopError> {
    match nvml.discover_gpus(PciInfo {
        bus: 0,
        bus_id: "".into(),
        device: 0,
        domain: 0,
        pci_device_id: 0,
        pci_sub_system_id: Some(0),
    }) {
        Ok(()) => {
            *have_fans = gpu_list
                .iter()
                .any(|gpu| gpu.inner.num_fans().unwrap_or(0) != 0);

            lh.debug(&format!("GPU has fans = {}", have_fans));
            lh.debug("Re-scanned PCI tree");
        }
        Err(e @ (NvmlError::OperatingSystem | NvmlError::NoPermission)) => {
            lh.debug(&format!("Failed to re-scan PCI tree: {e}"));
        }
        Err(e) => return Err(e.into()),
    }
    *gpu_list = crate::gpu::try_init_gpus(nvml, lh)?;
    if let ActiveTab::Gpu(i) = *active_tab
        && i >= gpu_list.len()
    {
        *active_tab = ActiveTab::Gpu(0);
    }
    Ok(())
}

/// Render the Master overview: all GPUs' main gauges in a grid, no PID window.
fn draw_master_view(f: &mut Frame<'_>, area: Rect, gpu_list: &[GpuInfo<'_>], have_fans: bool) {
    let block = Block::default()
        .title("NVTOP — Master Overview")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .border_type(BorderType::Rounded);
    f.render_widget(block, area);

    let inner = area.inner(Margin::new(1, 1));
    let n = gpu_list.len();
    if n == 0 {
        return;
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Percentage(100 / n as u16); n])
        .split(inner);

    for (i, gpu) in gpu_list.iter().enumerate() {
        draw_gpu_row(f, rows[i], gpu, have_fans);
    }
}

/// Render a single GPU's key metrics in a compact horizontal strip for the Master view.
fn draw_gpu_row(f: &mut Frame<'_>, area: Rect, gpu: &GpuInfo<'_>, have_fans: bool) {
    let cols = if have_fans { 6 } else { 5 };
    let constraints = vec![Constraint::Percentage(25); cols];
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);

    // Col 0: Card info (compact)
    let compute_cap = match gpu.inner.cuda_compute_capability() {
        Ok(cap) => format!("{}.{}", cap.major, cap.minor),
        Err(_) => "N/A".into(),
    };
    let card_text = format!(
        "[{}] {}\nDrv:{} CC:{}\n{}",
        gpu.index,
        truncate_str(&compact_name(&gpu.name), 18),
        gpu.driver_version,
        compute_cap,
        gpu.pcie_link,
    );
    f.render_widget(
        Paragraph::new(card_text).block(Block::default().borders(Borders::ALL).title("Card")),
        chunks[0],
    );

    // Col 1: Core utilisation
    let percent = gpu.inner.utilization_rates().map_or(0, |ur| ur.gpu as u16);
    f.render_widget(
        Gauge::default()
            .block(Block::default().borders(Borders::ALL).title("Util"))
            .gauge_style(
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
            .percent(percent)
            .label(format!("{}%", percent)),
        chunks[1],
    );

    // Col 2: Core clock
    let clk = gpu
        .inner
        .clock(Clock::Graphics, ClockId::Current)
        .unwrap_or(0);
    let clk_ratio = (clk as f64 / gpu.max_core_clock as f64).clamp(0.0, 1.0);
    f.render_widget(
        Gauge::default()
            .block(Block::default().borders(Borders::ALL).title("Clk"))
            .gauge_style(calculate_severity(clk_ratio).style_for())
            .ratio(clk_ratio)
            .label(format!("{}/{}M", clk, gpu.max_core_clock)),
        chunks[2],
    );

    // Col 3: Memory
    let mem_info = gpu.inner.memory_info().unwrap_or(MemoryInfo {
        free: 0,
        total: 0,
        used: 0,
        reserved: Default::default(),
        version: Default::default(),
    });
    let mem_used = mem_info.used as f64 / 1_073_741_824.0;
    let mem_total = mem_info.total as f64 / 1_073_741_824.0;

    let mem_pct = (mem_used / mem_total).clamp(0.0, 1.0);
    f.render_widget(
        Gauge::default()
            .block(Block::default().borders(Borders::ALL).title("Mem"))
            .gauge_style(calculate_severity(mem_pct).style_for())
            .ratio(mem_pct)
            .label(format!("{:.1}/{:.1}G", mem_used, mem_total)),
        chunks[3],
    );

    // Col 4: Temp
    let temp = gpu.inner.temperature(TemperatureSensor::Gpu).unwrap_or(0);
    let temp_ratio = (temp as f64 / 100.0).clamp(0.0, 1.0);
    f.render_widget(
        Gauge::default()
            .block(Block::default().borders(Borders::ALL).title("Tmp"))
            .gauge_style(calculate_severity(temp_ratio).style_for())
            .ratio(temp_ratio)
            .label(format!("{}°C", temp)),
        chunks[4],
    );

    // Col 5: Fan (if applicable)
    if have_fans {
        let nfans = gpu.inner.num_fans().unwrap_or(0);
        let fan_pct = if nfans > 0 {
            let avg = (0..nfans as usize)
                .flat_map(|v| gpu.inner.fan_speed(v as u32))
                .map(|u| u as f64)
                .sum::<f64>()
                / nfans as f64;
            (avg / 100.0).clamp(0.0, 1.0)
        } else {
            0.0
        };
        f.render_widget(
            Gauge::default()
                .block(Block::default().borders(Borders::ALL).title("Fan"))
                .gauge_style(calculate_severity(fan_pct).style_for())
                .ratio(fan_pct)
                .label(format!("{:.0}%", fan_pct * 100.0)),
            chunks[5],
        );
    }
}

/// Render the full single-GPU view (existing layout).
fn draw_single_gpu_view(
    f: &mut Frame<'_>,
    mid_area: Rect,
    gpu: &GpuInfo<'_>,
    have_fans: bool,
    fuzzy_search_input: &str,
    sort_by: &ProcessSortBy,
    sort_reverse: bool,
    process_selection_enabled: bool,
    highlighted_process_index: usize,
    selected_process_pid: Option<u32>,
    _show_process_view: bool,
) {
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

    // Top: Card info
    let top_chunk = main_layout[0];
    f.render_widget(draw_driver_info(gpu), top_chunk);

    // Middle: metrics
    let middle_area = main_layout[1];
    let middle_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(middle_area);

    let left_middle = Layout::default()
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(middle_chunks[0]);

    f.render_widget(draw_core_utilisation(gpu), left_middle[0]);
    f.render_widget(draw_core_clock(gpu).unwrap(), left_middle[1]);

    let right_middle = Layout::default()
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(middle_chunks[1]);

    f.render_widget(draw_memory_usage(gpu), right_middle[0]);
    f.render_widget(draw_gpu_die_temp(gpu), right_middle[1]);
    if have_fans {
        f.render_widget(draw_fan_speed(gpu), right_middle[2]);
    }

    // Bottom: processes
    let process_widget = draw_misc_with_processes(
        gpu,
        fuzzy_search_input,
        sort_by,
        sort_reverse,
        process_selection_enabled,
        highlighted_process_index,
        selected_process_pid,
    );
    f.render_widget(process_widget, main_layout[2]);
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max.saturating_sub(3)).collect::<String>() + "..."
    }
}

fn compact_name(name: &str) -> String {
    ["NVIDIA", "GeForce", "RTX", "GTX", "Quadro", "Tesla"]
        .iter()
        .fold(name.to_string(), |s, &w| {
            s.replace(w, "")
                .replace(&w.to_lowercase(), "")
                .replace(&w.to_uppercase(), "")
        })
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn draw_fan_speed<'d>(gpu: &GpuInfo<'d>) -> Gauge<'d> {
    let fans = gpu.inner.num_fans().unwrap_or(0);
    if fans == 0 {
        return Gauge::default();
    }

    let fan_pct = (0..fans)
        .filter_map(|v| gpu.inner.fan_speed(v).ok())
        .map(|u| u as f64)
        .sum::<f64>()
        / fans as f64
        / 100.0;

    let label = format!("{:.1}%", fan_pct * 100.0);
    let spanned_label = Span::styled(label, Style::new().white().bold().bg(Color::Black));

    Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Fan Speed"))
        .gauge_style(calculate_severity(fan_pct).style_for())
        .label(spanned_label)
        .ratio(fan_pct.clamp(0.0, 1.0))
}

fn draw_gpu_die_temp<'d>(gpu: &GpuInfo<'d>) -> Gauge<'d> {
    let gpu_die_temperature = gpu.inner.temperature(TemperatureSensor::Gpu).unwrap_or(0);

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
    let mem_info = gpu.inner.memory_info().unwrap_or(MemoryInfo {
        free: 0,
        total: 0,
        used: 0,
        reserved: Default::default(),
        version: Default::default(),
    });

    let mem_used = mem_info.used as f64 / 1_073_741_824.0;
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
        "Card: {:<24} | Driver: {:<12} | CUDA: {:<8} | Compute Cap: {} | PCIE: {} | Arch: {}",
        gpu.name,
        gpu.driver_version,
        gpu.cuda_version / 1000.0,
        compute_cap,
        gpu.pcie_link,
        gpu.arch,
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
        Ok(raw_processes) => {
            let mut processes = if let Some(isolated_pid) = selected_process_pid {
                raw_processes
                    .into_iter()
                    .filter(|p| p.pid == isolated_pid)
                    .collect()
            } else if !search_term.is_empty() {
                let mut scored_processes: Vec<(GpuProcess, i32)> = Vec::new();

                for proc in raw_processes {
                    let name_score = fuzzy_score(&proc.name, search_term);
                    let pid_score = fuzzy_score(&proc.pid.to_string(), search_term);
                    let max_score = name_score.max(pid_score);

                    if max_score > 0 {
                        scored_processes.push((proc, max_score));
                    }
                }

                scored_processes.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
                scored_processes.into_iter().map(|(proc, _)| proc).collect()
            } else {
                raw_processes
            };

            if processes.is_empty() {
                if selected_process_pid.is_some() {
                    format!(
                        "{}\n\nTarget PID {} not found",
                        gpu.misc,
                        selected_process_pid.unwrap()
                    )
                } else {
                    format!("{}\n\nNo processes running", gpu.misc)
                }
            } else {
                if search_term.is_empty() && selected_process_pid.is_none() {
                    match sort_by {
                        ProcessSortBy::Memory => {
                            if sort_reverse {
                                processes
                                    .sort_by_key(|process| std::cmp::Reverse(process.used_memory));
                            } else {
                                processes.sort_by_key(|a| a.used_memory);
                            }
                        }
                        ProcessSortBy::Name => {
                            if sort_reverse {
                                processes.sort_by(|a, b| {
                                    b.name.to_lowercase().cmp(&a.name.to_lowercase())
                                });
                            } else {
                                processes.sort_by(|a, b| {
                                    a.name.to_lowercase().cmp(&b.name.to_lowercase())
                                });
                            }
                        }
                        ProcessSortBy::Pid => {
                            if sort_reverse {
                                processes.sort_by_key(|process| std::cmp::Reverse(process.pid));
                            } else {
                                processes.sort_by_key(|a| a.pid);
                            }
                        }
                    }
                }

                if processes.is_empty() && !search_term.is_empty() {
                    format!("No processes match '{}'", search_term)
                } else {
                    let mut content = String::new();
                    content.push_str("Name                                    PID              Memory(MB)  Type\n");
                    for (idx, proc) in processes.iter().take(40).enumerate() {
                        let memory_mb = proc.used_memory / 1024 / 1024;
                        let process_type = if proc.is_compute { "C" } else { "G" };
                        let row_prefix = "  ";

                        let mut name_truncated = proc.name.clone();
                        if name_truncated.chars().count() > 38 {
                            name_truncated =
                                name_truncated.chars().take(35).collect::<String>() + "...";
                        }

                        let line = if process_selection_enabled && idx == highlighted_process_index
                        {
                            format!(
                                "> {:<38} {:>15} {:>13} MB  {}\n",
                                name_truncated, proc.pid, memory_mb, process_type
                            )
                        } else if selected_process_pid == Some(proc.pid) {
                            format!(
                                "* {:<38} {:>15} {:>13} MB  {}\n",
                                name_truncated, proc.pid, memory_mb, process_type
                            )
                        } else {
                            format!(
                                "{}{:<38} {:>15} {:>13} MB  {}\n",
                                row_prefix, name_truncated, proc.pid, memory_mb, process_type
                            )
                        };

                        content.push_str(&line);
                    }
                    content
                }
            }
        }
        Err(_) => format!("{}\n\nError retrieving processes", gpu.misc),
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

fn fuzzy_score(text: &str, pattern: &str) -> i32 {
    if pattern.is_empty() {
        return 0;
    }

    let text_lower = text.to_lowercase();
    let pattern_lower = pattern.to_lowercase();

    if !fuzzy_match(text, pattern) {
        return -1;
    }

    let mut score = 0;
    let text_chars: Vec<char> = text_lower.chars().collect();
    let pattern_chars: Vec<char> = pattern_lower.chars().collect();

    let mut text_idx = 0;
    let mut pattern_idx = 0;
    let mut consecutive_bonus = 0;

    while text_idx < text_chars.len() && pattern_idx < pattern_chars.len() {
        if text_chars[text_idx] == pattern_chars[pattern_idx] {
            if text_idx > 0
                && pattern_idx > 0
                && text_chars[text_idx - 1] == pattern_chars[pattern_idx - 1]
            {
                consecutive_bonus += 10;
            } else {
                consecutive_bonus = 10;
            }
            score += consecutive_bonus + 5;
            pattern_idx += 1;
        } else {
            consecutive_bonus = 0;
        }
        text_idx += 1;
    }

    score
}
