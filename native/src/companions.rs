// Small, ephemeral interactive reactions. Nothing is written to disk and
// merely petting a companion cannot launch a CLI or modify quota data.
use crate::components::logos::Provider;
use crate::ui::theme::color;
use eframe::egui::{Color32, Painter, Pos2, Rect, Stroke, Vec2};
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
    #[cfg(test)]
    demo_team: Option<Gesture>,
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
        #[cfg(test)]
        if let Some(gesture) = self.demo_team {
            return Some((gesture, 0.7)); // frozen animation frame for screenshots
        }
        Self::live(self.team)
    }

    pub fn team_label(&self) -> &'static str {
        match self.team().map(|(kind, _)| kind) {
            Some(Gesture::HighFive) => "high five",
            Some(Gesture::Gift) => "gift",
            Some(Gesture::Nap) => "nap time",
            _ => "team up",
        }
    }

    #[cfg(test)]
    pub fn demo_team(kind: Gesture) -> Self {
        Self { demo_team: Some(kind), ..Self::default() }
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

    /// Pixel-only reactions. Every stroke stays inside the pet stage.
    /// Captions are never painted over characters, meters or activity labels.
    pub fn decorate(&self, painter: &Painter, provider: Provider, area: Rect) {
        let Some((gesture, t)) = self.for_provider(provider) else {
            return;
        };
        let safe = area.shrink(5.0);
        let p = painter.with_clip_rect(safe);
        let fade = ((2.5 - t) / 0.6).clamp(0.0, 1.0);
        let tint = if provider == Provider::Claude { color::CLAUDE } else { color::CODEX };
        let color = tint.gamma_multiply(fade);
        let drift = (t * 2.0).sin() * 2.0;
        if matches!(gesture, Gesture::Heart | Gesture::Gift | Gesture::Nap) {
            pixel_heart(&p, Pos2::new(safe.right() - 12.0 + drift, safe.top() + 13.0 - t * 3.0), color::CLAUDE.gamma_multiply(fade));
        } else {
            for (i, (dx, dy)) in [(-17.0, -10.0), (15.0, -11.0), (-13.0, 13.0)].into_iter().enumerate() {
                let at = safe.center() + Vec2::new(dx, dy + (t * 6.0 + i as f32).sin() * 2.0);
                p.line_segment([at + Vec2::new(-2.0, 0.0), at + Vec2::new(2.0, 0.0)], Stroke::new(1.3, color));
                p.line_segment([at + Vec2::new(0.0, -2.0), at + Vec2::new(0.0, 2.0)], Stroke::new(1.3, color));
            }
        }
    }
    /// Keep the shared effect in a tiny lane between sprites; never
    /// draw long strokes or captions across the quota data.
    pub fn scene_between(&self, painter: &Painter, claude: Rect, codex: Rect) {
        let Some((gesture, t)) = self.team() else {
            return;
        };
        if claude == Rect::NOTHING || codex == Rect::NOTHING {
            return;
        }
        let vertical = (claude.center().y - codex.center().y).abs() > (claude.center().x - codex.center().x).abs();
        let at = if vertical {
            Pos2::new((claude.center().x + codex.center().x) / 2.0, (claude.bottom() + codex.top()) / 2.0 + 9.0)
        } else {
            Pos2::new((claude.right() + codex.left()) / 2.0, (claude.center().y + codex.center().y) / 2.0)
        };
        let slot = Rect::from_center_size(at, Vec2::splat(15.0));
        let p = painter.with_clip_rect(slot);
        let fade = ((2.5 - t) / 0.6).clamp(0.0, 1.0);
        match gesture {
            Gesture::HighFive => {
                // Two colored pixel paws meet with a small central spark.
                p.rect_filled(Rect::from_min_size(at + Vec2::new(-7.0, -1.0), Vec2::new(5.0, 5.0)), 0.0, color::CLAUDE.gamma_multiply(fade));
                p.rect_filled(Rect::from_min_size(at + Vec2::new(2.0, -1.0), Vec2::new(5.0, 5.0)), 0.0, color::CODEX.gamma_multiply(fade));
                for (a, b) in [(Vec2::new(-2.0, -5.0), Vec2::new(2.0, -5.0)), (Vec2::new(0.0, -7.0), Vec2::new(0.0, -3.0))] {
                    p.line_segment([at + a, at + b], Stroke::new(1.3, color::FG_STRONG.gamma_multiply(fade)));
                }
            }
            Gesture::Gift => {
                let bounce = (t * 7.0).sin().abs() * 2.0;
                let gift = Rect::from_center_size(at + Vec2::new(0.0, -bounce), Vec2::new(12.0, 11.0));
                p.rect_filled(gift, 0.0, color::CLAUDE.gamma_multiply(fade));
                p.rect_filled(Rect::from_center_size(gift.center(), Vec2::new(2.0, 11.0)), 0.0, color::FG_STRONG.gamma_multiply(fade));
                p.rect_filled(Rect::from_center_size(gift.center(), Vec2::new(12.0, 2.0)), 0.0, color::FG_STRONG.gamma_multiply(fade));
            }
            Gesture::Nap => pixel_heart(&p, at, color::CLAUDE.gamma_multiply(fade)),
            _ => {}
        }
    }
    /// Nap is an optional playful override only while already idle; never
    /// mislabel an active or quota-limited provider as sleeping.
    pub fn playful_mood(&self, mood: crate::components::pet::Mood) -> crate::components::pet::Mood {
        if matches!(self.team(), Some((Gesture::Nap, _))) && mood == crate::components::pet::Mood::Idle { crate::components::pet::Mood::Sleeping } else { mood }
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
