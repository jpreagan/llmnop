use ratatui::crossterm::style::{Attribute, ContentStyle};
use ratatui::style::{Color, Modifier, Style};
use std::fmt::Display;

pub const ACCENT: Color = Color::Cyan;

pub fn seconds(ms: f64) -> String {
    let s = ms / 1000.0;
    if s < 10.0 {
        format!("{s:.2}s")
    } else if s < 60.0 {
        format!("{s:.1}s")
    } else {
        format!(
            "{}m {:02}s",
            (s / 60.0).floor() as u64,
            (s % 60.0).floor() as u64
        )
    }
}

/// Inserts thousands separators into a whole number.
pub fn group(n: impl Display) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// ratatui 0.29 has no public conversion from its styles to crossterm's.
pub fn content_style(style: Style) -> ContentStyle {
    let mut content = ContentStyle {
        foreground_color: style.fg.map(Into::into),
        background_color: style.bg.map(Into::into),
        ..ContentStyle::default()
    };
    for (modifier, attribute) in [
        (Modifier::BOLD, Attribute::Bold),
        (Modifier::DIM, Attribute::Dim),
        (Modifier::ITALIC, Attribute::Italic),
        (Modifier::UNDERLINED, Attribute::Underlined),
        (Modifier::SLOW_BLINK, Attribute::SlowBlink),
        (Modifier::RAPID_BLINK, Attribute::RapidBlink),
        (Modifier::REVERSED, Attribute::Reverse),
        (Modifier::HIDDEN, Attribute::Hidden),
        (Modifier::CROSSED_OUT, Attribute::CrossedOut),
    ] {
        if style.add_modifier.contains(modifier) {
            content.attributes.set(attribute);
        }
    }
    content
}
