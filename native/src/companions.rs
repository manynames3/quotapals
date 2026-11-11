// Small, ephemeral interactive reactions. Nothing is written to disk and
// merely petting a companion cannot launch a CLI or modify quota data.
use crate::components::logos::Provider;
use crate::ui::theme::color;
use eframe::egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, Vec2};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    Heart,
    Wave,
    Dance,
    Cheer,
    WeeklyReset,
    Finished,
    HighFive,
    Gift,
    Nap,
}

impl Gesture {
    pub fn caption(self) -> &'static str {
        match self {
            Self::Heart => "loved!",
            Self::Wave => "hello!",
            Self::Dance => "dancing!",
            Self::Cheer => "5h reset!",
            Self::WeeklyReset => "weekly reset!",
            Self::Finished => "task done!",
            Self::HighFive => "high five!",
            Self::Gift => "a little gift!",
            Self::Nap => "nap buddies",
        }
    }
}

#[derive(Clone, Copy)]
pub struct Reaction {
    pub kind: Gesture,
    pub started: Instant,
}

#[cfg(test)]
pub static EMPTY: std::sync::LazyLock<Effects> = std::sync::LazyLock::new(Effects::new);

#[derive(Default)]
pub struct Effects {
    claude: Option<Reaction>,
    codex: Option<Reaction>,
    team: Option<Reaction>,
    clicks: [u32; 2],
    team_clicks: u32,
}

impl Effects {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn pet(&mut self, provider: Provider) {
        let idx = if provider == Provider::Codex { 1 } else { 0 };
        self.clicks[idx] = self.clicks[idx].wrapping_add(1);
        let kind = match self.clicks[idx] % 3 {
            1 => Gesture::Heart,
            2 => Gesture::Wave,
            _ => Gesture::Dance,
        };
        self.set(provider, kind);
    }
    fn set(&mut self, provider: Provider, kind: Gesture) {
        let slot = if provider == Provider::Codex { &mut self.codex } else { &mut self.claude };
        *slot = Some(Reaction { kind, started: Instant::now() });
    }
    /// Called only after a genuine reset transition was observed.
    pub fn celebrate_reset(&mut self, provider: Provider, weekly: bool) {
        self.set(provider, if weekly { Gesture::WeeklyReset } else { Gesture::Cheer });
    }

    pub fn celebrate_finished(&mut self, provider: Provider) {
        self.set(provider, Gesture::Finished);
    }
    pub fn team_up(&mut self) {
        let gesture = match self.team_clicks % 3 {
            0 => Gesture::HighFive,
            1 => Gesture::Gift,
            _ => Gesture::Nap,
        };
        self.team_clicks = self.team_clicks.wrapping_add(1);
        self.team = Some(Reaction { kind: gesture, started: Instant::now() });
    }
    fn live(item: Option<Reaction>) -> Option<(Gesture, f32)> {
        let item = item?;
        let t = item.started.elapsed().as_secs_f32();
        (t <= 2.5).then_some((item.kind, t))
    }
    fn for_provider(&self, provider: Provider) -> Option<(Gesture, f32)> {
        Self::live(if provider == Provider::Codex { self.codex } else { self.claude }).or_else(|| Self::live(self.team))
    }
    pub fn team(&self) -> Option<(Gesture, f32)> {
        Self::live(self.team)
    }
    pub fn animating(&self) -> bool {
        self.for_provider(Provider::Claude).is_some() || self.for_provider(Provider::Codex).is_some()
    }
    pub fn offset(&self, provider: Provider) -> Vec2 {
        match self.for_provider(provider) {
            Some((Gesture::Dance, t)) => Vec2::new((t * 14.0).sin() * 4.0, -(t * 22.0).sin().abs() * 3.0),
            Some((Gesture::Cheer | Gesture::WeeklyReset | Gesture::Finished, t)) => Vec2::new(0.0, -(t * 16.0).sin().abs() * 4.0),
            Some((Gesture::HighFive, t)) => Vec2::new((t * 8.0).sin() * if provider == Provider::Claude { 2.0 } else { -2.0 }, 0.0),
            _ => Vec2::ZERO,
        }
    }

    /// Draw animated hearts, waves, confetti and a small spoken reaction.
    /// All effects use native egui primitives; there is no GIF export.
    pub fn decorate(&self, p: &Painter, provider: Provider, area: Rect) {
        let Some((gesture, t)) = self.for_provider(provider) else {
            return;
        };
        let fade = ((2.5 - t) / 0.6).clamp(0.0, 1.0);
        let tint = if provider == Provider::Claude { color::CLAUDE } else { color::CODEX };
        let color = tint.gamma_multiply(fade);
        let center = area.center();
        let drift = (t * 2.0).sin() * 3.0;
        if matches!(gesture, Gesture::Heart | Gesture::Gift | Gesture::Nap) {
            pixel_heart(p, Pos2::new(center.x + 22.0 + drift, area.top() + 8.0 - t * 5.0), color::CLAUDE.gamma_multiply(fade));
        } else {
            // Four-pixel sparkles rotate around the pet.
            for (i, (dx, dy)) in [(-23.0, -11.0), (25.0, -15.0), (-19.0, 13.0)].into_iter().enumerate() {
                let at = center + Vec2::new(dx, dy + (t * 7.0 + i as f32).sin() * 4.0);
                p.line_segment([at + Vec2::new(-3.0, 0.0), at + Vec2::new(3.0, 0.0)], Stroke::new(1.3, color));
                p.line_segment([at + Vec2::new(0.0, -3.0), at + Vec2::new(0.0, 3.0)], Stroke::new(1.3, color));
            }
        }
        p.text(Pos2::new(center.x, area.top() - 5.0), Align2::CENTER_BOTTOM, gesture.caption(), FontId::proportional(9.0), color);
    }
    /// A shared interaction draws physical pixel props between both bodies
    /// instead of relying on an explanatory caption alone.
    pub fn scene_between(&self, p: &Painter, claude: Rect, codex: Rect) {
        let Some((gesture, t)) = self.team() else {
            return;
        };
        if claude == Rect::NOTHING || codex == Rect::NOTHING {
            return;
        }
        let vertical = (claude.center().y - codex.center().y).abs() > (claude.center().x - codex.center().x).abs();
        let (from, to) = if vertical {
            (Pos2::new(claude.center().x, claude.bottom() - 3.0), Pos2::new(codex.center().x, codex.top() + 3.0))
        } else {
            (Pos2::new(claude.right() - 5.0, claude.center().y + 9.0), Pos2::new(codex.left() + 5.0, codex.center().y + 9.0))
        };
        let at = Pos2::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
        let fade = ((2.5 - t) / 0.6).clamp(0.0, 1.0);
        match gesture {
            Gesture::HighFive => {
                let flex = (t * 8.0).sin().abs() * 3.0;
                let p1 = Pos2::new((from.x + at.x) / 2.0, (from.y + at.y) / 2.0);
                let p2 = Pos2::new((to.x + at.x) / 2.0, (to.y + at.y) / 2.0);
                p.line_segment([p1, at], Stroke::new(3.0, color::CLAUDE.gamma_multiply(fade)));
                p.line_segment([at, p2], Stroke::new(3.0, color::CODEX.gamma_multiply(fade)));
                p.line_segment([at + Vec2::new(-4.0 - flex, 0.0), at + Vec2::new(4.0 + flex, 0.0)], Stroke::new(1.5, color::FG_STRONG.gamma_multiply(fade)));
                p.line_segment([at + Vec2::new(0.0, -4.0 - flex), at + Vec2::new(0.0, 4.0 + flex)], Stroke::new(1.5, color::FG_STRONG.gamma_multiply(fade)));
            }
            Gesture::Gift => {
                let x = from.x + (to.x - from.x) * (t / 2.5).clamp(0.0, 1.0);
                let y = from.y + (to.y - from.y) * (t / 2.5).clamp(0.0, 1.0) - (t * 8.0).sin().abs() * 4.0;
                let box_at = Pos2::new(x, y);
                p.rect_filled(Rect::from_center_size(box_at, Vec2::new(13.0, 12.0)), 0.0, color::CLAUDE.gamma_multiply(fade));
                p.rect_filled(Rect::from_center_size(box_at, Vec2::new(3.0, 12.0)), 0.0, color::FG_STRONG.gamma_multiply(fade));
                p.rect_filled(Rect::from_center_size(box_at, Vec2::new(13.0, 2.0)), 0.0, color::FG_STRONG.gamma_multiply(fade));
            }
            Gesture::Nap => {
                pixel_heart(p, at + Vec2::new(0.0, -3.0 - (t * 2.0).sin() * 2.0), color::CLAUDE.gamma_multiply(fade));
            }
            _ => {}
        }
    }

    /// Nap is an optional playful override only while already idle; never
    /// mislabel an active or quota-limited provider as sleeping.
    pub fn playful_mood(&self, mood: crate::components::pet::Mood) -> crate::components::pet::Mood {
        if matches!(self.team(), Some((Gesture::Nap, _))) && mood == crate::components::pet::Mood::Idle { crate::components::pet::Mood::Sleeping } else { mood }
    }

    pub fn decorate_team(&self, p: &Painter, at: Pos2) {
        if let Some((gesture, t)) = self.team() {
            let fade = ((2.5 - t) / 0.6).clamp(0.0, 1.0);
            let tint = color::FG_STRONG.gamma_multiply(fade);
            p.text(
                at + Vec2::new(0.0, (t * 7.0).sin() * 2.0),
                Align2::CENTER_CENTER,
                match gesture {
                    Gesture::Gift => "a little gift!",
                    Gesture::Nap => "Zz...",
                    _ => "high five!",
                },
                FontId::proportional(14.0),
                tint,
            );
        }
    }
}

pub fn pixel_heart(p: &Painter, at: Pos2, color: Color32) {
    for (dx, dy, w) in [(-5.0, -2.0, 3.0), (1.0, -2.0, 3.0), (-6.0, 1.0, 12.0), (-4.0, 4.0, 8.0), (-2.0, 7.0, 4.0)] {
        p.rect_filled(Rect::from_min_size(at + Vec2::new(dx, dy), Vec2::new(w, 3.0)), 0.0, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interactions_cycle_independently() {
        let mut x = Effects::new();
        x.pet(Provider::Claude);
        assert_eq!(x.for_provider(Provider::Claude).unwrap().0, Gesture::Heart);
        assert!(x.for_provider(Provider::Codex).is_none());
        x.pet(Provider::Codex);
        x.pet(Provider::Codex);
        assert_eq!(x.for_provider(Provider::Codex).unwrap().0, Gesture::Wave);
        x.team_up();
        assert_eq!(x.team().unwrap().0, Gesture::HighFive);
        x.team_up();
        assert_eq!(x.team().unwrap().0, Gesture::Gift);
        x.team_up();
        assert_eq!(x.team().unwrap().0, Gesture::Nap);
        assert_eq!(x.playful_mood(crate::components::pet::Mood::Idle), crate::components::pet::Mood::Sleeping);
        assert_eq!(x.playful_mood(crate::components::pet::Mood::Working), crate::components::pet::Mood::Working);
    }
}
