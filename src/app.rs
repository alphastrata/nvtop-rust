use log::trace;
use nvml_wrapper::enum_wrappers::device::{Clock, ClockId};
use nvml_wrapper::{enum_wrappers::device::TemperatureSensor, struct_wrappers::device::MemoryInfo};

use ratatui::{prelude::*, widgets::*};
use ratatui::{
    prelude::{CrosstermBackend, Terminal},
    widgets::Paragraph,
};

use std::time::Duration;

use crate::stylers::calculate_severity;
use crate::{errors, gpu::GpuInfo};
pub type Frame<'a> = ratatui::Frame<'a, CrosstermBackend<std::io::Stderr>>;

pub fn run(gpu: GpuInfo, delay: Duration) -> anyhow::Result<(), errors::NvTopError> {
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(std::io::stderr(), crossterm::terminal::EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stderr()))?;

    loop {
        terminal.draw(|f| {
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

            // Chunking into |A  |B|:
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(vec![Constraint::Percentage(70), Constraint::Percentage(30)])
                .margin(1)
                .split(f.size());

            {
                let chunks = Layout::default()
                    .constraints(vec![
                        Constraint::Percentage(44),
                        Constraint::Percentage(44),
                        Constraint::Percentage(12),
                    ])
                    .margin(1)
                    .split(chunks[0]);

                // Chunk |A|
                // Core:
                let core_guage = draw_core_utilisation(&gpu);
                f.render_widget(core_guage, chunks[0]);

                // Core Clock:
                let core_guage = draw_core_clock(&gpu);
                f.render_widget(core_guage, chunks[1]);

                //NVIDIA TITAN RTX  Driver Version: 535.113.01   CUDA Version: 12.2
                let paragraph = draw_misc(&gpu);
                f.render_widget(paragraph, chunks[2]);
            }

            {
                //Chunking into|A  |B
                //             |a2 |C
                //             |   |D
                let chunks = Layout::default()
                    .constraints([
                        Constraint::Percentage(33),
                        Constraint::Percentage(33),
                        Constraint::Percentage(33),
                    ])
                    .direction(Direction::Vertical)
                    .margin(1)
                    .split(chunks[1]);

                // Chunk B
                // Memory:
                let mem_usage_guage = draw_memory_usage(&gpu);
                f.render_widget(mem_usage_guage, chunks[0]);

                // Chunk C:
                // Temp:
                let temp_guage = draw_gpu_die_temp(&gpu);
                f.render_widget(temp_guage, chunks[1]);

                // Chunk D:
                // Fanspeed:
                let fan_guage = draw_fan_speed(&gpu);
                f.render_widget(fan_guage, chunks[2]);
            }
        })?;

        if crossterm::event::poll(std::time::Duration::from_millis(250))? {
            if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                if key.code == crossterm::event::KeyCode::Char('q') {
                    break;
                }
            }
        }
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

    let label = format!("{:.2}%", avg);
    let spanned_label = Span::styled(label, Style::new().white().bold());
    let fan_guage = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Fan Speed"))
        .gauge_style(calculate_severity(avg).style_for())
        .label(spanned_label)
        .set_style(Style::default())
        .ratio((avg / 100.).clamp(0., 1.0));
    fan_guage
}

fn draw_gpu_die_temp<'d>(gpu: &GpuInfo<'d>) -> Gauge<'d> {
    let gpu_die_temperature = gpu
        .inner
        .temperature(TemperatureSensor::Gpu)
        .map_or(0, |temp| temp);

    let label = format!("{:.2}°C", gpu_die_temperature);
    let spanned_label = Span::styled(label, Style::new().white().bold());
    let temp_ratio = (gpu_die_temperature as f64 / 100.).clamp(0., 1.0);
    let temp_guage = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Temp"))
        .gauge_style(calculate_severity(gpu_die_temperature as f32).style_for())
        .label(spanned_label)
        .set_style(Style::default())
        .ratio(temp_ratio);
    temp_guage
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

    let mem_used = mem_info.used as f64 / 1_073_741_824.0;
    // as GB
    let mem_total = mem_info.total as f64 / 1_073_741_824.0;
    let mem_percentage = mem_used / mem_total;
    // Normalize to a value between 0 and 1

    let label = format!("{:.2}/{:.2}GB", mem_percentage, mem_total);
    let spanned_label = Span::styled(label, Style::new().white().bold());
    let mem_usage_guage = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Memory Usage"))
        .gauge_style(Style {
            fg: Some(Color::LightGreen),
            bg: None,
            underline_color: Some(Color::DarkGray),
            add_modifier: Modifier::BOLD,
            sub_modifier: Modifier::UNDERLINED,
        })
        .label(spanned_label)
        .ratio(mem_percentage.clamp(0., 1.0));
    mem_usage_guage
}

fn draw_misc<'d>(gpu: &GpuInfo<'d>) -> Paragraph<'d> {
    let card = gpu.inner.brand().unwrap();
    let driver = gpu
        .inner
        .nvml()
        .sys_driver_version()
        .map_or("UNAVAILABLE".into(), |driver| driver);

    let cuda_v = gpu
        .inner
        .nvml()
        .sys_cuda_driver_version()
        .map_or(0.0, |sdv| sdv as f32);

    //TODO: self.brand self.sys_driver, sys_cuda // because these never change we may as well get them at init and use em everywhere...
    let label = format!(
        "Card: {:?}    Driver Version: {}    CUDA Version: {}",
        card,
        driver,
        cuda_v / 1000.0
    );

    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        "Misc",
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD),
    ));

    let spanned_label = Span::styled(label, Style::new().white().bold());
    let paragraph = Paragraph::new(spanned_label)
        .block(block)
        .wrap(Wrap { trim: true });
    paragraph
}

fn draw_core_utilisation<'d>(gpu: &GpuInfo<'d>) -> Gauge<'d> {
    let utilisation_rates = gpu.inner.utilization_rates();
    let percent = utilisation_rates.map_or(0, |ur| ur.gpu as u16);

    let core_guage = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Core Utilisation"),
        )
        .gauge_style(Style {
            fg: Some(Color::Green),
            bg: None,
            underline_color: Some(Color::DarkGray),
            add_modifier: Modifier::BOLD,
            sub_modifier: Modifier::UNDERLINED,
        })
        .percent(percent);
    core_guage
}

fn draw_core_clock<'d>(gpu: &GpuInfo<'d>) -> Gauge<'d> {
    let core_clock = gpu.inner.clock(Clock::Graphics, ClockId::Current);
    let max_clock = gpu.inner.clock(Clock::Graphics, ClockId::CustomerMaxBoost);
    let percent = core_clock.map_or(0, |ur| (ur / max_clock.unwrap_or(1)) as u16);
    trace!("core_clock {}", percent);
    let label = format!("{}MHz", percent);
    let spanned_label = Span::styled(label, Style::new().white().bold());

    let core_guage = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Core Clock"))
        .gauge_style(Style {
            fg: Some(Color::Green),
            bg: None,
            underline_color: Some(Color::DarkGray),
            add_modifier: Modifier::BOLD,
            sub_modifier: Modifier::UNDERLINED,
        })
        .label(spanned_label)
        .percent(percent.clamp(0, 1));
    core_guage
}
