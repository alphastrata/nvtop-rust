use log::{trace, debug};
use nvml_wrapper::enum_wrappers::device::{Clock, ClockId};
use nvml_wrapper::error::NvmlError;
use nvml_wrapper::struct_wrappers::device::PciInfo;
use nvml_wrapper::{enum_wrappers::device::TemperatureSensor, struct_wrappers::device::MemoryInfo};

use ratatui::{prelude::*, widgets::*};
use ratatui::{
    prelude::{CrosstermBackend, Terminal},
    widgets::Paragraph,
};

use std::time::Duration;

use crate::errors::NvTopError;
use crate::stylers::calculate_severity;
use crate::{errors, gpu::GpuInfo};
pub type Frame<'a> = ratatui::Frame<'a, CrosstermBackend<std::io::Stderr>>;

pub fn run(nvml: nvml_wrapper::Nvml, delay: Duration) -> anyhow::Result<(), errors::NvTopError> {
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(std::io::stderr(), crossterm::terminal::EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stderr()))?;
    trace!("crossterm initialisation successful");

    let mut gpu_list = crate::gpu::list_available_gpus(&nvml)?;
    let mut selected_gpu: usize = 0;

    loop {
        _ = terminal.draw(|f| {
            f.render_widget(Paragraph::new("q to quit"), f.size());

            let top_chunk = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(100)])
                .split(f.size());

            // Outermost Block, which draws the green border aound the whole UI.
            let block = Block::default()
                .title("NVTOP")
                .title_position(block::Position::Top)
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green))
                .border_type(BorderType::Rounded)
                .style(Style::default());
            f.render_widget(block, top_chunk[0]);

            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(vec![Constraint::Percentage(70), Constraint::Percentage(30)])
                .margin(1)
                .split(f.size());

            let gpu = &gpu_list[selected_gpu];

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

                // Misc:
                let paragraph = draw_misc(gpu);
                f.render_widget(paragraph, chunks[2]);
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

                // Fanspeed:
                let fan_gauge = draw_fan_speed(gpu);
                f.render_widget(fan_gauge, chunks[2]);
            }

            
        })?;

        if crossterm::event::poll(std::time::Duration::from_millis(250))? {
            if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                use crossterm::event::KeyCode;

                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::F(n) if gpu_list.len() > n.into() => selected_gpu = n.into(),
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
                            Ok(()) => debug!("Re-scanned PCI tree"),
                            Err(e @ (NvmlError::OperatingSystem | NvmlError::NoPermission)) => {
                                debug!("Failed to re-scan PCI tree: {e}");
                            }
                            Err(e) => return Err(e.into()),
                        }
                    
                        // re-scan for devices
                        gpu_list = crate::gpu::list_available_gpus(&nvml)?;
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
    let temps = gpu.inner.num_fans().map_or(0, |fc| fc);
    let avg = (0..temps as usize)
        .flat_map(|v| gpu.inner.fan_speed(v as u32))
        .map(|u| u as f64)
        .sum::<f64>()
        / temps as f64;

    let percentage = (avg / 100.).clamp(0., 1.0);
    let label = format!("{:.1}%", avg);
    let spanned_label = Span::styled(label, Style::new().white().bold().bg(Color::Black));

    Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Fan Speed"))
        .gauge_style(calculate_severity(percentage).style_for())
        .label(spanned_label)
        .set_style(Style::default())
        .ratio(percentage)
}

fn draw_gpu_die_temp<'d>(gpu: &GpuInfo<'d>) -> Gauge<'d> {
    let gpu_die_temperature = gpu
        .inner
        .temperature(TemperatureSensor::Gpu)
        .map_or(0, |temp| temp);

    let label = format!("{:.2}°C", gpu_die_temperature);
    let spanned_label = Span::styled(label, Style::new().white().bold().bg(Color::Black));
    let temp_ratio = (gpu_die_temperature as f64 / 100.).clamp(0., 1.0);

    Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Temp"))
        .gauge_style(calculate_severity(temp_ratio).style_for())
        .label(spanned_label)
        .set_style(Style::default())
        .ratio(temp_ratio)
}

fn draw_memory_usage<'d>(gpu: &GpuInfo<'d>) -> Gauge<'d> {
    let mem_info = gpu.inner.memory_info().map_or(
        MemoryInfo {
            free: 0,
            total: 0, //TODO: This never changes so put in self
            used: 0,
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
