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
        (244, 11, 104), // Blueish
        (221, 244, 11), // Greenish
        (11, 244, 151), // Yellowish
        (34, 11, 244),  // Pinkish
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
            Severity::Low => &styles[0],
            Severity::Medium => &styles[1],
            Severity::High => &styles[2],
            Severity::Critical => &styles[3],
        }
    }
}

/// Work out the %ile that `n` is in out of 0..100
pub fn calculate_severity<N>(n: N) -> Severity
where
    N: PartialEq
        + PartialOrd
        + std::ops::Div<Output = N>
        + std::ops::Mul<Output = N>
        + From<f32>
        + std::fmt::Display,
{
    // Check which quartile `n` falls into and return the corresponding severity
    match n {
        _ if n < N::from(0.6) => Severity::Low,
        _ if n < N::from(0.75) => Severity::Medium,
        _ if n < N::from(0.85) => Severity::High,
        _ => Severity::Critical,
    }
}
