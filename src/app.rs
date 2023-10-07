use crate::gpu::GpuInfo;

use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
use ratatui::{
    prelude::{CrosstermBackend, Terminal},
    widgets::Paragraph,
};
use std::time::Duration;

use ratatui::{prelude::*, widgets::*};
pub type Frame<'a> = ratatui::Frame<'a, CrosstermBackend<std::io::Stderr>>;

pub fn run(gpu: GpuInfo, delay: Duration) -> anyhow::Result<(), Box<dyn std::error::Error>> {
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
                .constraints(vec![Constraint::Percentage(60), Constraint::Percentage(40)])
                .margin(1)
                .split(f.size());

            {
                let chunks = Layout::default()
                    .constraints(vec![Constraint::Percentage(80), Constraint::Percentage(20)])
                    .margin(1)
                    .split(chunks[0]);

                // Chunk |A|
                // Core:
                let percent = gpu
                    .inner
                    .utilization_rates()
                    .unwrap()
                    .gpu
                    .try_into()
                    .unwrap();

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

                f.render_widget(core_guage, chunks[0]);

                //NVIDIA TITAN RTX  Driver Version: 535.113.01   CUDA Version: 12.2
                let card = gpu.inner.brand().unwrap();
                let driver = gpu.inner.nvml().sys_driver_version().unwrap();
                let cuda_v = gpu.inner.nvml().sys_cuda_driver_version().unwrap();
                let label = format!(
                    "Card: {:?}    Driver Version: {}    CUDA Version: {}",
                    card,
                    driver,
                    cuda_v as f32 / 1000.0
                );

                let block = Block::default().borders(Borders::ALL).title(Span::styled(
                    "Misc",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ));
                let paragraph = Paragraph::new(label).block(block).wrap(Wrap { trim: true });
                f.render_widget(paragraph, chunks[1]);
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
                let mem_info = gpu.inner.memory_info().unwrap();

                let mem_used = mem_info.used as f64 / 1_073_741_824.0;
                let mem_total = mem_info.total as f64 / 1_073_741_824.0;
                let mem_percentage = mem_used / mem_total; // Normalize to a value between 0 and 1
                let label = format!("{:.2}/{:.2}GB", mem_percentage, mem_total);
                let mem_usage_guage = Gauge::default()
                    .block(Block::default().borders(Borders::ALL).title("Memory Usage"))
                    .gauge_style(Style {
                        fg: Some(Color::LightGreen),
                        bg: None,
                        underline_color: Some(Color::DarkGray),
                        add_modifier: Modifier::BOLD,
                        sub_modifier: Modifier::UNDERLINED,
                    })
                    .label(label)
                    .ratio(mem_percentage.clamp(0., 1.0));

                f.render_widget(mem_usage_guage, chunks[0]);

                // Chunk C:
                // Temp:
                let temps = gpu.inner.temperature(TemperatureSensor::Gpu).unwrap();
                let label = format!("{:.2}°C", temps);
                let temp_ratio = (temps as f64 / 100.).clamp(0., 1.0);
                let temp_guage = Gauge::default()
                    .block(Block::default().borders(Borders::ALL).title("Temp"))
                    .gauge_style(Style {
                        fg: Some(Color::LightYellow),
                        bg: None,
                        underline_color: Some(Color::Red),
                        add_modifier: Modifier::BOLD,
                        sub_modifier: Modifier::UNDERLINED,
                    })
                    .label(label)
                    .style(Style::default())
                    .ratio(temp_ratio);

                f.render_widget(temp_guage, chunks[1]);

                // Chunk D:
                // Fanspeed:
                let temps = gpu.inner.num_fans().unwrap();
                let avg = (0..temps as usize)
                    .into_iter()
                    .flat_map(|v| gpu.inner.fan_speed(v as u32))
                    .map(|u| u as f64)
                    .sum::<f64>()
                    / temps as f64;

                let label = format!("{:.2}%", avg);
                let fan_guage = Gauge::default()
                    .block(Block::default().borders(Borders::ALL).title("Fan Speed"))
                    .gauge_style(Style {
                        fg: Some(Color::LightYellow),
                        bg: None,
                        underline_color: Some(Color::Red),
                        add_modifier: Modifier::BOLD,
                        sub_modifier: Modifier::UNDERLINED,
                    })
                    .label(label)
                    .style(Style::default())
                    .ratio((avg / 100.).clamp(0., 1.0));

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
