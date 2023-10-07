#![allow(unused_imports, dead_code, unused_variables)]
use std::time::Duration;

use crate::gpu::GpuInfo;
use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
use ratatui::{
    prelude::{CrosstermBackend, Terminal},
    widgets::Paragraph,
};

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

            // Outermost Block:
            let block = Block::default()
                .title("nvtop")
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
                .split(f.size());

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

            {
                //Chunking into|A  |B
                //             |a2 |C
                //             |   |D
                let chunks = Layout::default()
                    .constraints([
                        Constraint::Percentage(50),
                        Constraint::Percentage(25),
                        Constraint::Percentage(25),
                    ])
                    .direction(Direction::Vertical)
                    .split(chunks[1]);

                // Chunk B
                // Memory:
                let mem_info = gpu.inner.memory_info().unwrap();

                let mem_used = mem_info.used as f64 / 1_073_741_824.0;
                let mem_total = mem_info.total as f64 / 1_073_741_824.0;
                let mem_percentage = mem_used / mem_total; // Normalize to a value between 0 and 1

                let label = format!("{:.2}/{:.2}GB", mem_percentage, mem_total);
                let precent: u16 = (mem_used * 100.0) as u16; // Convert back to a percentage
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
                    .ratio(mem_percentage.clamp(0., 1.0));

                f.render_widget(temp_guage, chunks[1]);

                // Chunk D:
                // Fanspeed:
                let temps = gpu.inner.num_fans().unwrap();
                let avg: f32 = (0..temps as usize)
                    .into_iter()
                    .flat_map(|v| gpu.inner.fan_speed(v as u32))
                    .map(|u| u as f32)
                    .sum::<f32>()
                    / temps as f32;

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
                    .ratio(mem_percentage.clamp(0., 1.0));

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
