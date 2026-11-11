// Compact: a floating pet with every connected service's usage windows
// visible together. No provider tab needs to be opened to read the limits.

use eframe::egui::{Align, Align2, Context, FontId, Pos2, Rect, Ui, Vec2};

use super::{Action, Frame, MARGIN, Size, connected, sessions, shown};
use crate::components::account::account;
use crate::components::logos::Provider;
use crate::components::meter;
use crate::components::service_row::ServiceRow;
use crate::components::status::caption;
use crate::format::countdown;
use crate::ui::bar::bar;
use crate::ui::controls::{button_row, click_area};
use crate::ui::icon;
use crate::ui::surface::{card, halo};
use crate::ui::text::text_mid;
use crate::ui::theme::{color, text};

const CARD_W: f32 = 332.0;
const PAD_TOP: f32 = 32.0;
const PAD: f32 = 12.0;
const STAGE: Vec2 = Vec2::new(64.0, 70.0);
const LINE_H: f32 = 14.0;
const GROUP_H: f32 = 78.0;
const GROUP_GAP: f32 = 6.0;
const REMIND_H: f32 = 28.0;

fn rows_h(rows: usize) -> f32 {
    rows as f32 * GROUP_H + rows.saturating_sub(1) as f32 * GROUP_GAP
}

/// One service uses two rows, one for the five-hour window and one for the week.
pub fn size(rows: usize, reminder: bool) -> [f32; 2] {
    let content_h = (rows_h(rows) + LINE_H).max(STAGE.y);
    let card_h = PAD_TOP + content_h + PAD + if reminder { REMIND_H } else { 0.0 };
    [CARD_W + 2.0 * MARGIN, card_h + 2.0 * MARGIN]
}

pub fn reminder(f: &Frame) -> Option<bool> {
    if !f.snap.loaded {
        return None;
    }
    crate::components::reminder::state(f.snap, shown(f), f.session, f.heads_up)
}

/// Pet-only's hover glance uses the original per-provider session summary.
pub fn rows(f: &Frame) -> Vec<ServiceRow> {
    sessions(f.snap, f.session).into_iter().map(|(provider, pct, _)| ServiceRow { provider, pct }).collect()
}

fn ext(w: Option<&crate::codex::Window>) -> (Option<f64>, Option<i64>) {
    let pct = w.and_then(|w| w.pct);
    (pct, if pct.is_some() { w.and_then(|w| w.reset_ms) } else { None })
}

/// Neither an expired limit nor an unrecorded one should be displayed as zero.
fn windows(f: &Frame, provider: Provider) -> [(Option<f64>, Option<i64>); 2] {
    match provider {
        Provider::Claude => {
            let week = f.snap.limits.as_ref().and_then(|l| meter::live(&l.week, f.snap.limits_at));
            [f.session.map_or((None, None), |(p, t)| (Some(p), t)), week.map_or((None, None), |(p, t)| (Some(p), t))]
        }
        Provider::Codex => {
            let Some(c) = &f.snap.codex else { return [(None, None); 2] };
            if c.weekly_only { [(None, None), ext(c.session.as_ref())] } else { [ext(c.session.as_ref()), ext(c.weekly.as_ref())] }
        }
        Provider::Cursor => {
            let Some(c) = &f.snap.cursor else { return [(None, None); 2] };
            [ext(c.session.as_ref()), ext(c.api.as_ref())]
        }
    }
}

fn draw_window(p: &eframe::egui::Painter, left: f32, right: f32, top: f32, label: &str, value: (Option<f64>, Option<i64>), provider: Provider) {
    let (pct, reset) = value;
    let at = top + 8.0;
    p.text(Pos2::new(left + 5.0, at), Align2::LEFT_CENTER, label, FontId::proportional(text::SMALL), color::FG_LABEL);
    let num = pct.map(|v| format!("{v:.0}%")).unwrap_or_else(|| "—".to_string());
    let (fill, fg) = meter::level(pct.unwrap_or(0.0), provider.bar());
    p.text(Pos2::new(left + 54.0, at), Align2::LEFT_CENTER, &num, FontId::proportional(text::VALUE), fg);
    let time = match (pct, reset) {
        (Some(_), Some(ms)) => format!("reset in {}", countdown(ms)),
        (Some(_), None) => "reset unknown".to_string(),
        (None, _) => "awaiting reading".to_string(),
    };
    p.text(Pos2::new(right, at), Align2::RIGHT_CENTER, time, FontId::proportional(text::SMALL), color::FG_SUB);
    let track = Rect::from_min_size(Pos2::new(left + 5.0, top + 20.0), Vec2::new(right - left - 5.0, 3.0));
    bar(p, track, pct.map(|v| (v / 100.0) as f32), fill, color::TRACK_THIN);
}

pub fn draw(ui: &Ui, ctx: &Context, f: &Frame) -> Vec<Action> {
    let mut actions = vec![];
    let rect = f.area;
    card(ui, ctx, f.card);
    let p = ui.painter();

    let band_y = rect.top() + 15.5;
    account(p, rect.left() + 10.0, band_y, f.snap.profile.as_ref(), !f.snap.loaded);
    match button_row(ui, rect.right() - 8.0, band_y, &[("close", icon::CLOSE), ("expand", icon::EXPAND), ("pet-only", icon::PET)]) {
        Some(0) => actions.push(Action::Close),
        Some(1) => actions.push(Action::Resize(Size::Expanded)),
        Some(2) => actions.push(Action::Resize(Size::PetOnly)),
        _ => {}
    }

    let remind = reminder(f);
    let below = if remind.is_some() { REMIND_H } else { 0.0 };
    let content = Rect::from_min_max(Pos2::new(rect.left() + PAD, rect.top() + PAD_TOP), Pos2::new(rect.right() - PAD, rect.bottom() - PAD - below));
    if let Some(on) = remind
        && crate::components::reminder::button_right(ui, content.right(), content.bottom() + 6.0, on)
    {
        actions.push(Action::Remind(shown(f), !on));
    }

    let line_y = content.bottom() - 6.0;
    let above = Rect::from_min_max(content.left_top(), Pos2::new(content.right(), line_y - LINE_H / 2.0 - 2.0));
    let pet_area = Rect::from_center_size(Pos2::new(above.left() + STAGE.x / 2.0, above.center().y), STAGE);
    halo(p, pet_area, color::CLAUDE_BAR.gamma_multiply(0.06));
    let body = f.pet.draw_fit(p, pet_area.shrink2(Vec2::new(2.0, 2.0)), f.mood);
    let activity = f.snap.usage.as_ref().and_then(|u| u.activity.as_deref());
    caption(p, ctx, f.logos, pet_area.center().x, line_y, f.mood, activity, &super::workers(f.snap));

    let left = above.left() + STAGE.x + 8.0;
    let right = above.right();
    let services = connected(f.snap);
    let mut y = above.center().y - rows_h(services.len()) / 2.0;
    for provider in services {
        if click_area(ui, Rect::from_min_size(Pos2::new(left, y), Vec2::new(right - left, GROUP_H)), ("compact-provider", provider.name())).clicked() {
            actions.push(Action::SelectProvider(provider));
        }
        let tint = provider.tint();
        let logo = Rect::from_center_size(Pos2::new(left + 9.0, y + 10.0), Vec2::splat(12.0));
        f.logos.draw(p, provider, logo, tint);
        p.text(Pos2::new(left + 21.0, y + 10.0), Align2::LEFT_CENTER, provider.name(), FontId::proportional(text::BODY), color::FG_STRONG);

        let rows = windows(f, provider);
        let names = if provider == Provider::Cursor { ["Plan", "API"] } else { ["5h", "Week"] };
        draw_window(p, left, right, y + 19.0, names[0], rows[0], provider);
        draw_window(p, left, right, y + 47.0, names[1], rows[1], provider);
        y += GROUP_H + GROUP_GAP;
    }

    if !f.snap.loaded {
        text_mid(p, right, line_y, Align::Max, "reading local usage…", text::TINY, color::FG_MUTED);
    } else {
        text_mid(p, right, line_y, Align::Max, "usage used · time to reset", text::TINY, color::FG_RESET);
    }

    if let Some((said, progress)) = f.bubble {
        let tip = Pos2::new(body.center().x + 8.0, body.top() + 4.0);
        if crate::components::bubble::bubble(ui, said, tip, 18.0, rect.width() - 24.0, rect.shrink(6.0), progress) {
            actions.push(Action::DismissBubble);
        }
    }
    actions
}

#[cfg(test)]
mod tests {
    #[test]
    fn two_services_show_both_limits_without_tabs() {
        assert!(super::size(2, false)[1] > super::size(1, false)[1]);
        assert_eq!(super::size(2, false)[0], super::size(1, false)[0]);
    }
}
