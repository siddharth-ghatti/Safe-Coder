//! Shimmer animation effect for thinking/loading status
//!
//! Inspired by OpenAI Codex CLI - creates a smooth animated highlight
//! that sweeps across text to indicate processing.

use ratatui::{
    style::{Color, Style},
    text::Span,
};
use std::time::Instant;

// Module-level start time for animation synchronization
lazy_static::lazy_static! {
    static ref START_TIME: Instant = Instant::now();
}

/// Get elapsed time since module initialization (for consistent animation)
fn elapsed_since_start() -> std::time::Duration {
    START_TIME.elapsed()
}

/// Create shimmer-animated spans for the given text
///
/// The shimmer creates a smooth highlight band that sweeps across the text,
/// making it clear that the AI is actively processing.
pub fn shimmer_spans(text: &str, base_color: Color, highlight_color: Color) -> Vec<Span<'static>> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return vec![];
    }

    let period = chars.len();
    let sweep_seconds = 2.0f32; // 2-second sweep cycle
    let band_half_width = 5.0f32; // Width of the highlight band

    // Calculate current sweep position (wraps around)
    let elapsed = elapsed_since_start().as_secs_f32();
    let pos = (elapsed % sweep_seconds) / sweep_seconds * (period as f32);

    let mut spans = Vec::with_capacity(chars.len());

    for (i, ch) in chars.iter().enumerate() {
        let dist = ((i as f32) - pos).abs();

        // Calculate highlight intensity using smooth cosine falloff
        let intensity = if dist <= band_half_width {
            0.5 * (1.0 + (std::f32::consts::PI * dist / band_half_width).cos())
        } else {
            0.0
        };

        // Blend colors based on intensity
        let color = if intensity > 0.0 {
            blend_colors(base_color, highlight_color, intensity)
        } else {
            base_color
        };

        spans.push(Span::styled(ch.to_string(), Style::default().fg(color)));
    }

    spans
}

/// Create a simple shimmer indicator (pulsing dot or spinner)
pub fn shimmer_indicator(frame: usize) -> Span<'static> {
    const INDICATORS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let idx = frame % INDICATORS.len();

    // Pulse the color based on frame
    let intensity = ((frame as f32 * 0.3).sin() * 0.5 + 0.5) as f32;
    let color = blend_colors(
        Color::Rgb(100, 140, 200), // Base blue
        Color::Rgb(120, 200, 220), // Highlight cyan
        intensity,
    );

    Span::styled(INDICATORS[idx], Style::default().fg(color))
}

/// Blend two RGB colors based on intensity (0.0 = base, 1.0 = highlight)
fn blend_colors(base: Color, highlight: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);

    match (base, highlight) {
        (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
            Color::Rgb(
                lerp_u8(r1, r2, t),
                lerp_u8(g1, g2, t),
                lerp_u8(b1, b2, t),
            )
        }
        _ => if t > 0.5 { highlight } else { base }
    }
}

/// Linear interpolation for u8
fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    ((a as f32) + ((b as f32) - (a as f32)) * t) as u8
}

/// Create a thinking status line with shimmer effect
pub fn thinking_status(message: &str, frame: usize) -> Vec<Span<'static>> {
    let mut spans = vec![shimmer_indicator(frame), Span::raw(" ")];

    spans.extend(shimmer_spans(
        message,
        Color::Rgb(150, 150, 160), // Secondary text
        Color::Rgb(120, 200, 220), // Cyan highlight
    ));

    spans
}
