// Status words and indicators used by the expanded provider view.

use eframe::egui::{Align, Color32, Painter, Pos2, Vec2};

use super::pet::Mood;
use crate::format::tokens;
use crate::ui::skeleton::sk_line;
use crate::ui::tag::{TagStyle, tag};
use crate::ui::text::{bold_mid, text_mid};
use crate::ui::theme::{color, text};

pub const STATUS_H: f32 = 34.0;

/// The word for the mood; while working, what Claude is doing.
pub fn status_word(m: Mood, activity: Option<&str>) -> String {
    match m {
        Mood::Working => activity.unwrap_or("working").to_string(),
        Mood::Fire => "on fire".into(),
        Mood::Tired => "maxed out".into(),
        Mood::Sleeping => "sleeping".into(),
        Mood::Idle => "idle".into(),
    }
}

/// (dot, word) colours for a mood.
pub fn mood_colors(m: Mood) -> (Color32, Color32) {
    match m {
        Mood::Working => (color::WORKING, color::WORKING),
        Mood::Fire => (color::FIRE, color::FIRE_TEXT),
        Mood::Tired => (color::TIRED, color::TIRED_TEXT),
        _ => (color::REST, color::FG),
    }
}

/// Expanded: dot, word, today's tokens, and the live tag when the % is real.
pub fn status_line(p: &Painter, left: f32, right: f32, y: f32, mood: Mood, activity: Option<&str>, today: Option<u64>, live: bool) {
    let (dot, word_color) = mood_colors(mood);
    p.circle_filled(Pos2::new(left + 3.5, y), 3.5, dot);
    let r = bold_mid(p, left + 13.0, y, Align::Min, &status_word(mood, activity), text::BODY, word_color);
    if today.is_none() {
        // still loading: hold the tokens' place
        sk_line(p, r.right() + 10.0, y, 110.0, 8.0);
    }
    if let Some(n) = today {
        let sep = text_mid(p, r.right() + 6.0, y, Align::Min, "·", text::BODY, color::FG_MUTED.gamma_multiply(0.5));
        text_mid(p, sep.right() + 6.0, y, Align::Min, &format!("{} tokens today", tokens(n)), text::SMALL, color::FG_MUTED);
    }
    if live {
        let style = TagStyle { fg: color::LIVE, edge: color::LIVE_EDGE, wash: color::LIVE_WASH, pad: Vec2::new(5.0, 3.0) };
        tag(p, right, y, Align::Max, "live", &style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_word_is_the_activity_while_working() {
        assert_eq!(status_word(Mood::Working, Some("running")), "running");
        assert_eq!(status_word(Mood::Working, None), "working");
        assert_eq!(status_word(Mood::Idle, Some("running")), "idle");
    }
}
