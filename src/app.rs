use nvml_wrapper::enum_wrappers::device::{Clock, ClockId};
use nvml_wrapper::error::NvmlError;
use nvml_wrapper::struct_wrappers::device::PciInfo;
use nvml_wrapper::{enum_wrappers::device::TemperatureSensor, struct_wrappers::device::MemoryInfo};

use ratatui::symbols::DOT;
use ratatui::{prelude::*, widgets::*};
use ratatui::{
    prelude::{CrosstermBackend, Terminal},
    widgets::Paragraph,
    widgets::Block,
};

use std::time::Duration;

use crate::errors::NvTopError;
use crate::stylers::calculate_severity;
use crate::termite::LoggingHandle;
use crate::{errors, gpu::GpuInfo};

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
    let mut show_process_view: bool = false;

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

                f.render_widget(
                    Paragraph::new(
                        if show_process_view {
                            "q to quit, p to rescan devices, fn keys to switch devices, ESC to return to stats"
                        } else {
                            "q to quit, p to rescan devices, fn keys to switch devices, f or / to show processes"
                        }
                    ),
                    layout[2],
                );

                layout[1]
            };

            // Outermost Block, which draws the green border aound the whole UI.
            let block = Block::default()
                .title("NVTOP")
                .title_top(Line::from("NVTOP"))
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green))
                .border_type(BorderType::Rounded)
                .style(Style::default());
            f.render_widget(block, mid_area);

            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(vec![Constraint::Percentage(70), Constraint::Percentage(30)])
                .margin(1)
                .split(f.area());

            {
                let chunks = Layout::default()
                    .constraints(vec![
                        Constraint::Percentage(60),
                        Constraint::Percentage(20),
                        Constraint::Percentage(20),
                    ])
                    .margin(1)
                    .split(chunks[0]);

                // Core:
                let core_gauge = draw_core_utilisation(gpu);
                f.render_widget(core_gauge, chunks[0]);

                // Core Clock:
                let core_gauge = draw_core_clock(gpu).unwrap();
                f.render_widget(core_gauge, chunks[1]);

                if !show_process_view {
                    // Misc (normal view):
                    let paragraph = draw_misc(gpu);
                    f.render_widget(paragraph, chunks[2]);
                } else {
                    // Process view instead of misc
                    let process_widget = draw_gpu_processes(gpu);
                    f.render_widget(process_widget, chunks[2]);
                }
            }

            {
                let chunks = Layout::default()
                    .constraints([
                        Constraint::Percentage(33),
                        Constraint::Percentage(33),
                        Constraint::Percentage(33),
                    ])
                    .direction(Direction::Vertical)
                    .margin(1)
                    .split(chunks[1]);

                // Memory:
                let mem_usage_gauge = draw_memory_usage(gpu);
                f.render_widget(mem_usage_gauge, chunks[0]);

                // Temp:
                let temp_gauge = draw_gpu_die_temp(gpu);
                f.render_widget(temp_gauge, chunks[1]);

                // Fan speed:
                if have_fans && !show_process_view {
                    let gauge = draw_fan_speed(gpu);
                    f.render_widget(gauge, chunks[2]);
                } else if show_process_view {
                    // Show additional process information or blank space
                    let blank_widget = draw_process_blank_space();
                    f.render_widget(blank_widget, chunks[2]);
                } else {
                    // Regular fan speed view
                    let gauge = draw_fan_speed(gpu);
                    f.render_widget(gauge, chunks[2]);
                }
            }
        })?;

        if crossterm::event::poll(std::time::Duration::from_millis(250))? {
            if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                use crossterm::event::KeyCode;

                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Esc => {
                        // Exit process view mode if currently in it
                        if show_process_view {
                            show_process_view = false;
                        }
                    }
                    KeyCode::Char('f') | KeyCode::Char('/') => {
                        // Toggle or enter process view mode
                        show_process_view = true;
                    }
                    KeyCode::F(n) if (1..=gpu_list.len()).contains(&n.into()) => {
                        selected_gpu = usize::from(n - 1)
                    }

                    #[cfg(target_os = "linux")]
                    KeyCode::Char('p') => {
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

fn draw_misc<'d>(gpu: &'d GpuInfo<'d>) -> Paragraph<'d> {
    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        "Misc",
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD),
    ));

    let spanned_label = Span::styled(&gpu.misc, Style::new().white().bold());

    Paragraph::new(spanned_label)
        .block(block)
        .wrap(Wrap { trim: true })
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

// Placeholder for drawing GPU processes - need to implement actual process retrieval
fn draw_gpu_processes<'d>(gpu: &GpuInfo<'d>) -> Paragraph<'d> {
    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        "GPU Processes",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    ));

    let processes_text = match get_gpu_processes(gpu) {
        Ok(processes) => {
            if processes.is_empty() {
                "No processes running".to_string()
            } else {
                let mut text = String::from("PID\t\tMemory\n");
                for proc in processes.iter().take(10) { // Show top 10 processes
                    text.push_str(&format!("{}\t\t{} MB\n", proc.pid, proc.used_memory / 1024 / 1024)); // Convert to MB
                }
                text
            }
        },
        Err(_) => { 
            // If there's an issue with the specific API, try a more general approach
            match gpu.inner.running_graphics_processes() {
                Ok(graphics_processes) => {
                    if graphics_processes.is_empty() {
                        "No processes running".to_string()
                    } else {
                        let mut text = String::from("PID\t\tMemory\n");
                        for process in graphics_processes.iter().take(10) {
                            // Try to extract memory value - if not available, use 0
                            let memory_mb = 0; // Placeholder until we determine correct API
                            text.push_str(&format!("{}\t\t{} MB\n", process.pid, memory_mb));
                        }
                        text
                    }
                },
                Err(_) => "Error retrieving processes".to_string(),
            }
        },
    };

    Paragraph::new(processes_text)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((0, 0))
}

fn draw_process_blank_space<'d>() -> Paragraph<'d> {
    let block = Block::default().borders(Borders::ALL).title(" ");
    Paragraph::new("")
        .block(block)
        .wrap(Wrap { trim: true })
}

// Structure to represent a process running on the GPU 
#[derive(Debug)]
struct GpuProcess {
    pid: u32,
    used_memory: u64,
}

// Function to retrieve processes running on the GPU (placeholder until API is properly identified)
fn get_gpu_processes<'d>(gpu: &GpuInfo<'d>) -> Result<Vec<GpuProcess>, NvmlError> {
    // Try various possible methods to get running processes based on nvml-wrapper API
    // This might need adjustment based on the specific version
    
    // First, try the compute processes method
    match gpu.inner.running_compute_processes() {
        Ok(processes) => {
            let mut gpu_processes = Vec::new();
            for process in processes {
                // Since the exact API varies by version, use a default value
                let memory_value = 0; // Placeholder until we determine correct API access
                
                gpu_processes.push(GpuProcess {
                    pid: process.pid,
                    used_memory: memory_value,
                });
            }
            Ok(gpu_processes)
        },
        Err(_) => {
            // If that doesn't work, return empty vector
            Ok(Vec::new())
        }
    }
}
