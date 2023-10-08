use ratatui::prelude::*;

// Define an enum for severity levels
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    const COLORS: [(u8, u8, u8); 4] = [
        (66, 84, 245),  // Blueish
        (44, 135, 26),  // Greenish
        (217, 148, 30), // Orangeish
        (212, 22, 8),   // Reddish
    ];

    pub fn style_for(&self) -> Style {
        // Create styles for each severity level
        let styles = Self::COLORS
            .iter()
            .map(|(r, g, b)| {
                Style::default()
                    .fg(Color::Rgb(*r, *g, *b))
                    .add_modifier(Modifier::BOLD | Modifier::ITALIC)
            })
            .collect::<Vec<Style>>();

        *match self {
            Severity::Low => &styles[0],      // Pink
            Severity::Medium => &styles[1],   // Green
            Severity::High => &styles[2],     // Orange
            Severity::Critical => &styles[3], //Red
        }
    }
}

/// Work out the %ile that `n` is in out of 0..100
pub fn calculate_severity<N>(n: N) -> Severity
where
    N: Into<f64>,
{
    let clamped_n = n.into().clamp(0.0, 1.0);

    match clamped_n {
        _ if clamped_n < 0.40 => Severity::Low,
        _ if clamped_n < 0.70 => Severity::Medium,
        _ if clamped_n < 0.80 => Severity::High,
        _ => Severity::Critical,
    }
}
