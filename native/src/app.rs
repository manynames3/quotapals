// The app shell: it owns the pet, the logos, the poll and the remembered
// view; each frame it works out the mood, hands a screen its data, and
// applies what the screen returns. It repaints only when the pet's next frame
// is due, a reading lands, or a clock ticks over.

use eframe::egui::{self, Context, Vec2};
use std::time::{Duration, Instant};

use crate::components::logos::Logos;
use crate::components::meter::live;
use crate::components::pet::{Mood, Pet};
use crate::poller::{self, Command, Settings, Shared, Snapshot};
use crate::prefs::Prefs;
use crate::screens::{Action, Frame, MARGIN, Size, View, compact, expanded, pet_only, settings};
use crate::ui::{motion, theme};
use crate::usage::Usage;
use std::sync::mpsc::Sender;

pub struct App {
    pet: Pet,
    effects: crate::companions::Effects,
    history: crate::reset_history::History,
    was_working: [bool; 2],
    logos: Logos,
    data: Shared,
    commands: Sender<Command>,
    /// the login code being pasted
    code: String,
    /// what config.json holds, and the copy Settings edits
    prefs: Prefs,
    draft: Prefs,
    settings: Settings,
    pinned: Option<Mood>,
    view: View,
    size: [f32; 2],
    started: Instant,
    shot_sent: bool,
    /// the cursor, watched while just the pet is up
    cursor: crate::platform::cursor::Watch,
    /// whether clicks fall through the window (just the pet, off the pet)
    passthrough: bool,
    /// the menu-bar icon, while Clauddy lives there
    tray: Option<crate::platform::tray::Tray>,
    /// the panel is open under the menu-bar icon; `focused` once it took focus
    popover: bool,
    popover_focused: bool,
    /// the window is shown (it hides while Clauddy lives in the menu bar)
    shown: bool,
    /// the screen on show (Settings tab, size) and when it came in, to fade it in
    screen: (Option<settings::Tab>, Size),
    entered: Instant,
    /// the panel fading away on its way to the menu bar
    leaving: Option<Instant>,
    /// the window as asked of the OS (larger than `size` while the card morphs)
    window: [f32; 2],
    /// the pixels per point the window was last sized at
    sized_at: f32,
    /// Last zoom used to size the native viewport.
    sized_zoom: i32,
    /// Original zoom while the user drags the bottom-right resize grip.
    zoom_drag_start: Option<i32>,
    /// Latest *total* drag distance (egui's drag_delta is per frame).
    zoom_drag_delta: Vec2,
    /// the pet's voice: what it noticed, and what it's saying now
    talk: crate::talk::Talk,
    updater: crate::updates::Updater,
    heard: u64,
    said: Option<Said>,
    /// the card's size as it shows now, and the morph under way (from, started)
    card_now: Vec2,
    morph: Option<(Vec2, Instant)>,
}

/// Working wins; otherwise the session's heat; otherwise rest.
pub fn mood_for(usage: Option<&Usage>, session_pct: Option<f64>, fire: f64) -> Mood {
    if usage.is_some_and(|u| u.active) {
        return Mood::Working;
    }
    match session_pct {
        Some(p) if p >= 100.0 => Mood::Tired,
        Some(p) if p >= fire => Mood::Fire,
        _ if usage.is_some_and(|u| u.sleeping) => Mood::Sleeping,
        _ => Mood::Idle,
    }
}

/// The mood across every connected service: any of them working wins; with
/// none working, the hottest session sets it (an expired Codex or Cursor
/// reading doesn't count; Cursor's session is its billing month). Claude's
/// logs alone say when it's asleep.
pub fn mood_across(snap: &Snapshot, claude: Option<(f64, Option<i64>)>, fire: f64) -> Mood {
    if !crate::screens::workers(snap).is_empty() {
        return Mood::Working;
    }
    let hottest = crate::screens::sessions(snap, claude).into_iter().filter_map(|s| s.1).fold(None, |m: Option<f64>, p| Some(m.map_or(p, |m| m.max(p))));
    mood_for(snap.usage.as_ref(), hottest, fire)
}

/// A remark on screen: the words for each size, and when it goes.
struct Said {
    text: String,
    short: String,
    since: Instant,
    until: Instant,
}

impl Said {
    /// 0..1 as it appears and goes (0.15 s each way)
    fn shown(&self) -> f32 {
        let now = Instant::now();
        if now >= self.until {
            return (1.0 - now.duration_since(self.until).as_secs_f32() / 0.15).max(0.0);
        }
        (now.duration_since(self.since).as_secs_f32() / 0.15).min(1.0)
    }
}

/// The window's size at launch, from the remembered view, before any reading.
/// At normal zoom, macOS keeps a transparent canvas so switching panels
/// never blinks. At larger/smaller user scales, use a snug viewport instead:
/// otherwise the canvas grows taller than the display and may clip the UI.
const FIXED_CANVAS: bool = cfg!(target_os = "macos");
/// Wide enough for the expanded panel, tall enough for its Details open
/// or any Settings tab; it grows if a screen ever needs more.
const CANVAS: [f32; 2] = [352.0, 780.0];

pub fn initial_size() -> [f32; 2] {
    // the menu bar's popover is an ordinary window the card's size
    let prefs = Prefs::load();
    if FIXED_CANVAS && !prefs.menu_bar && prefs.zoom == 100 {
        return CANVAS;
    }
    let view = load_view();
    let size = if prefs.menu_bar { Size::Expanded } else { view.size };
    match size {
        Size::Compact => compact::size(1, false),
        Size::Expanded => expanded::size(&Snapshot::default(), &view, None, 80.0),
        Size::PetOnly => pet_only::size(),
    }
}

fn view_path() -> std::path::PathBuf {
    crate::auth::default_data_dir().join("native-ui.json")
}

fn load_view() -> View {
    std::fs::read(view_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_else(|| View { size: Size::Compact, ..View::default() })
}

fn save_view(v: &View) {
    if let Ok(body) = serde_json::to_string_pretty(v) {
        let _ = std::fs::write(view_path(), body);
    }
}

/// Open a URL in the default browser.
pub fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let (cmd, args) = ("open", vec![url]);
    #[cfg(target_os = "windows")]
    let (cmd, args) = ("cmd", vec!["/C", "start", "", url]);
    #[cfg(all(unix, not(target_os = "macos")))]
    let (cmd, args) = ("xdg-open", vec![url]);
    let _ = std::process::Command::new(cmd).args(args).spawn();
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install_font(&cc.egui_ctx);
        crate::alerts::ask_permission();
        crate::platform::login::open_at_login();
        crate::platform::leftovers::clear();
        let settings = Settings::load();
        let (data, commands) = poller::spawn(cc.egui_ctx.clone(), settings.clone());
        crate::auto_continue::spawn(data.clone());
        App {
            pet: Pet::new(&cc.egui_ctx),
            effects: crate::companions::Effects::new(),
            history: crate::reset_history::History::load(),
            was_working: [false; 2],
            logos: Logos::load(&cc.egui_ctx),
            data,
            commands,
            code: String::new(),
            prefs: Prefs::load(),
            draft: Prefs::load(),
            settings,
            pinned: Mood::from_env(),
            view: load_view(),
            size: [0.0, 0.0],
            started: Instant::now(),
            shot_sent: false,
            cursor: crate::platform::cursor::Watch::new(&cc.egui_ctx),
            passthrough: false,
            tray: None,
            popover: false,
            popover_focused: false,
            shown: !Prefs::load().menu_bar,
            screen: (None, Size::Expanded),
            entered: Instant::now(),
            leaving: None,
            window: initial_size(),
            sized_at: 0.0,
            sized_zoom: 0,
            zoom_drag_start: None,
            zoom_drag_delta: Vec2::ZERO,
            updater: crate::updates::Updater::new(&cc.egui_ctx),
            talk: crate::talk::Talk::new(Some(crate::auth::default_data_dir().join("native-voice.json"))),
            heard: 0,
            said: None,
            card_now: Vec2::ZERO,
            morph: None,
        }
    }

    fn apply(&mut self, ctx: &Context, actions: Vec<Action>) {
        for a in actions {
            match a {
                Action::Close => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                Action::Pet(provider) => self.effects.pet(provider),
                Action::TeamUp => self.effects.team_up(),
                Action::Resize(s) => {
                    // from Settings, minimize and pet-only leave it the way Cancel does
                    if self.view.settings.take().is_some() {
                        self.draft = self.prefs.clone();
                        self.view.pet_picker = false;
                        self.view.account_menu = false;
                    }
                    self.view.size = s;
                }
                Action::OpenUsagePage => open_url("https://claude.ai/settings/usage"),
                Action::ToggleDetails => self.view.details = !self.view.details,
                Action::ToggleModels => self.view.models = !self.view.models,
                Action::ToggleProjects => self.view.projects = !self.view.projects,
                Action::ToggleAccountMenu => {
                    self.view.account_menu = !self.view.account_menu;
                    self.view.pending_remove = None;
                }
                Action::CloseAccountMenu => {
                    self.view.account_menu = false;
                    self.view.pending_remove = None;
                }
                Action::SwitchAccount(id) => {
                    self.view.account_menu = false;
                    self.send(Command::Switch(id));
                }
                Action::AskRemoveAccount(id) => self.view.pending_remove = Some(id),
                Action::RemoveAccount(id) => {
                    self.view.pending_remove = None;
                    self.send(Command::Remove(id));
                }
                Action::AddAccount => {
                    self.view.account_menu = false;
                    self.code.clear();
                    self.send(Command::Add);
                }
                Action::BeginLogin => {
                    self.code.clear();
                    self.send(Command::BeginLogin);
                }
                Action::ToggleSettings => {
                    // opening starts a fresh draft; closing drops unsaved edits
                    self.draft = self.prefs.clone();
                    self.view.settings = if self.view.settings.is_some() { None } else { Some(settings::Tab::Accounts) };
                    self.view.pet_picker = false;
                    self.view.account_menu = false;
                }
                Action::SettingsTab(t) => self.view.settings = Some(t),
                Action::EditPrefs(p) => self.draft = p,
                Action::SaveSettings => {
                    // moving to the menu bar: let the panel fade out first
                    if self.draft.menu_bar && !self.prefs.menu_bar {
                        self.leaving = Some(Instant::now());
                    }
                    let reopen_floating = self.prefs.menu_bar && self.draft.menu_bar_alongside;
                    self.draft.save();
                    self.prefs = self.draft.clone();
                    if reopen_floating {
                        self.show(ctx, true);
                    }
                    self.settings.fire_threshold = self.prefs.fire as f64;
                    self.send(Command::Prefs(self.prefs.clone()));
                    self.view.settings = None;
                }
                Action::Logout => self.send(Command::Logout),
                Action::SelectProvider(p) => self.view.provider = p,
                Action::SetCodex(on) => {
                    // connecting a service takes effect at once, like logging in
                    self.prefs.codex = on;
                    self.draft.codex = on;
                    self.prefs.save();
                    self.send(Command::Prefs(self.prefs.clone()));
                    if on {
                        self.view.provider = crate::components::logos::Provider::Codex;
                    }
                }
                Action::Remind(p, on) => self.send(Command::Remind(p, on)),
                Action::HidePopover => self.close_popover(ctx),
                Action::CheckUpdates => self.updater.check(),
                Action::TogglePetPicker => self.view.pet_picker = !self.view.pet_picker,
                Action::InstallUpdate => self.updater.install(),
                Action::DismissBubble => {
                    if let Some(s) = &mut self.said {
                        s.until = s.until.min(Instant::now());
                    }
                }
                Action::SetCursor(on) => {
                    self.prefs.cursor = on;
                    self.draft.cursor = on;
                    self.prefs.save();
                    self.send(Command::Prefs(self.prefs.clone()));
                    if on {
                        self.view.provider = crate::components::logos::Provider::Cursor;
                    }
                }
                Action::SubmitCode => {
                    let code = std::mem::take(&mut self.code);
                    self.send(Command::CompleteLogin(code));
                }
            }
            save_view(&self.view);
        }
    }

    /// Hear one poll. A remark is said when the voice is on, nothing covers
    /// the pet (Settings, pet-only), and the gap allows; only headlines blip,
    /// and never in the menu bar or the compact face, nor while muted.
    fn listen(&mut self, snap: &Snapshot, mood: crate::components::pet::Mood, session: Option<(f64, Option<i64>)>, size: Size) {
        let now = crate::usage::now_ms();
        let hour = chrono::Timelike::hour(&chrono::Local::now());
        let seen = crate::talk::Seen { snap, mood, claude: session, fire: self.settings.fire_threshold, hour };
        let Some(r) = self.talk.listen(&seen, now) else { return };
        let quiet = !self.prefs.talk || self.view.settings.is_some() || size == Size::PetOnly;
        if quiet || !self.talk.allow(&r, now) {
            return;
        }
        let words = if size == Size::Compact { &r.line.short } else { &r.line.text };
        let start = Instant::now();
        let stay = Duration::from_millis((words.len() as u64 * 70 + 2500).max(4500));
        self.said = Some(Said { text: r.line.text.clone(), short: r.line.short.clone(), since: start, until: start + stay });
        if r.headline && self.prefs.sound && !self.prefs.menu_bar && size == Size::Expanded && self.prefs.muted_until <= now {
            crate::voice::play(&r.line.text, r.tone);
        }
    }

    /// Standard Mac keyboard alternatives when dragging a corner is
    /// inconvenient; the selected size is the same persisted setting.
    fn keyboard_scale(&mut self, ctx: &Context) {
        if self.prefs.menu_bar {
            return;
        }
        let step = ctx.input(|i| {
            if !i.modifiers.command {
                return 0;
            }
            if i.key_pressed(egui::Key::Num0) {
                i32::MAX // special marker for 100% reset
            } else if i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals) {
                10
            } else if i.key_pressed(egui::Key::Minus) {
                -10
            } else {
                0
            }
        });
        if step != 0 {
            self.prefs.zoom = if step == i32::MAX { 100 } else { (self.prefs.zoom + step).clamp(75, 250) };
            self.draft.zoom = self.prefs.zoom;
            self.prefs.save();
            ctx.request_repaint();
        }
    }

    /// Calculate the requested scale from a *frozen* drag start. We must not
    /// resize the native viewport while the pointer is held: the egui drag
    /// coordinates depend on zoom, so changing zoom on each drag frame causes
    /// a feedback loop that rapidly oscillates the window size.
    fn drag_zoom(baseline: i32, delta: Vec2) -> i32 {
        let raw = baseline as f32 + (delta.x + delta.y) * 0.48;
        ((raw / 5.0).round() as i32 * 5).clamp(75, 250)
    }

    /// Borderless macOS windows lack a native resize border; resize by
    /// dragging this affordance. Preview a target without changing any layout
    /// during the drag, and commit once on mouse release.
    fn resize_grip(&mut self, ui: &egui::Ui, ctx: &Context, card: egui::Rect) {
        let grip = egui::Rect::from_min_size(card.right_bottom() - Vec2::splat(25.0), Vec2::splat(22.0));
        let response = ui.interact(grip, ui.scope_id().with("scale-grip"), egui::Sense::click_and_drag());
        let c = if response.hovered() || response.dragged() { theme::color::FG } else { theme::color::FG_MUTED };
        let corner = card.right_bottom() - Vec2::splat(10.0);
        let paint = ui.painter().with_clip_rect(card.shrink(2.0));
        for offset in [0.0, 5.0, 10.0] {
            paint.line_segment([corner + Vec2::new(-offset - 2.0, 0.0), corner + Vec2::new(0.0, -offset - 2.0)], egui::Stroke::new(1.3, c));
        }
        if response.hovered() || response.dragged() {
            ctx.set_cursor_icon(egui::CursorIcon::ResizeNwSe);
        }
        if response.drag_started() {
            self.zoom_drag_start = Some(self.prefs.zoom);
            self.zoom_drag_delta = Vec2::ZERO;
        }
        // egui::Response::drag_delta is only the change in the latest frame
        // and returns ZERO on release. Retain the total drag while held so
        // mouse-up applies the full motion, not the final (zero) frame.
        if let Some(delta) = response.total_drag_delta() {
            self.zoom_drag_delta = delta;
        }
        // The tooltip previews the target; no window resizing while held.
        if let Some(baseline) = self.zoom_drag_start {
            let preview = Self::drag_zoom(baseline, self.zoom_drag_delta);
            let _ = response.clone().on_hover_text(format!("Release to apply {preview}%"));
        } else {
            let _ = response.clone().on_hover_text("Drag to resize · release to apply");
        }
        if response.drag_stopped()
            && let Some(baseline) = self.zoom_drag_start.take()
        {
            let next = Self::drag_zoom(baseline, self.zoom_drag_delta);
            self.zoom_drag_delta = Vec2::ZERO;
            if next != self.prefs.zoom {
                self.prefs.zoom = next;
                self.draft.zoom = next;
                self.prefs.save();
                ctx.request_repaint();
            }
        }
        if response.clicked() {
            self.prefs.zoom = if self.prefs.zoom >= 250 { 100 } else { (self.prefs.zoom + 10).min(250) };
            self.draft.zoom = self.prefs.zoom;
            self.prefs.save();
            ctx.request_repaint();
        }
    }

    fn show(&mut self, ctx: &Context, on: bool) {
        if on != self.shown {
            self.shown = on;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(on));
        }
    }

    /// Open the panel under the menu-bar icon (`icon`: its box, physical px).
    fn open_popover(&mut self, ctx: &Context, icon: Option<egui::Rect>) {
        let ppp = ctx.pixels_per_point();
        if let Some(r) = icon {
            let x = r.center().x / ppp - self.size[0] / 2.0; // the card's width, centred under the icon
            let y = r.bottom() / ppp + 2.0;
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(x.round(), y.round())));
        }
        self.popover = true;
        self.popover_focused = false;
        if self.passthrough {
            self.passthrough = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::MousePassthrough(false));
        }
        self.entered = Instant::now();
        self.show(ctx, true);
        crate::platform::activate();
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    fn close_popover(&mut self, ctx: &Context) {
        self.popover = false;
        self.view.account_menu = false;
        // always say it: eframe shows a window after its first frame even
        // when it was built hidden, so `shown` can't be trusted here
        self.shown = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
    }

    /// Floating or in the menu bar: put the icon up or take it down, and
    /// answer what was clicked there.
    fn menu_bar(&mut self, ctx: &Context, snap: &Snapshot, session: Option<(f64, Option<i64>)>) {
        use crate::platform::tray::{Tray, TrayEvent, line};
        // In "Both" mode the macOS status icon and draggable window coexist.
        let wanted = (self.prefs.menu_bar || self.prefs.menu_bar_alongside) && self.leaving.is_none();
        if wanted && self.tray.is_none() {
            self.tray = Tray::new(ctx);
            if self.tray.is_some() && self.prefs.menu_bar {
                self.close_popover(ctx);
            }
        } else if !wanted && self.tray.is_some() {
            self.tray = None;
            self.popover = false;
            self.show(ctx, true);
        }
        let Some(tray) = &mut self.tray else { return };
        // the session closest to its limit; 🔥 past the fire threshold
        let sessions: Vec<_> = crate::screens::sessions(snap, session).into_iter().map(|s| s.1).collect();
        let text = crate::platform::tray::title_hottest(&sessions, self.settings.fire_threshold);
        tray.set_title(&text);
        // the menu leads with the limits, as the panel does
        let mut lines = vec![line("Claude 5h", session.map(|s| s.0), session.and_then(|s| s.1))];
        if let Some(l) = &snap.limits {
            let week = live(&l.week, snap.limits_at);
            lines.push(line("Claude weekly", week.map(|w| w.0), week.and_then(|w| w.1)));
            let (five, weekly) = self.history.counts().pair(false);
            lines.push(format!("Claude resets observed · 5h {five} · weekly {weekly}"));
            for sc in &l.scoped {
                let w = live(&crate::auth::Window { pct: sc.pct, reset_ms: sc.reset_ms }, snap.limits_at);
                lines.push(line(&format!("{} weekly", sc.label), w.map(|w| w.0), w.and_then(|w| w.1)));
            }
        }
        // the other services, each with its own windows
        let ext = |w: &Option<crate::codex::Window>| (w.as_ref().and_then(|w| w.pct), w.as_ref().and_then(|w| w.reset_ms));
        if let Some(c) = &snap.codex {
            let (p, r) = ext(&c.session);
            lines.push(line(if c.weekly_only { "Codex weekly" } else { "Codex session" }, p, r));
            if !c.weekly_only {
                let (p, r) = ext(&c.weekly);
                lines.push(line("Codex weekly", p, r));
                let (five, weekly) = self.history.counts().pair(true);
                lines.push(format!("Codex resets observed · 5h {five} · weekly {weekly}"));
            }
        }
        if let Some(c) = &snap.cursor {
            let (p, r) = ext(&c.session);
            lines.push(line("Cursor usage", p, r));
            let (p, r) = ext(&c.api);
            lines.push(line("Cursor API", p, r));
        }
        tray.set_lines(lines);
        let events = tray.take_events();
        let icon = tray.rect();
        for e in events {
            match e {
                TrayEvent::Open if self.prefs.menu_bar_alongside => {
                    // The widget is already there: bring it forward rather
                    // than turning the same window into a separate popover.
                    self.show(ctx, true);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                TrayEvent::Open => self.open_popover(ctx, icon),
                TrayEvent::Floating => {
                    self.prefs.menu_bar = false;
                    self.prefs.menu_bar_alongside = false;
                    self.draft.menu_bar = false;
                    self.draft.menu_bar_alongside = false;
                    self.prefs.save();
                }
            }
        }
        // a click anywhere else takes focus away: the popover goes
        let focused = ctx.input(|i| i.viewport().focused);
        if self.popover && focused == Some(true) {
            self.popover_focused = true;
        }
        if self.popover && self.popover_focused && focused == Some(false) {
            self.close_popover(ctx);
        }
    }

    fn send(&self, cmd: Command) {
        let _ = self.commands.send(cmd);
    }

    // CLAUDDY_SHOT=path.png: save one frame after 8 s, then quit (dev aid)
    fn shoot(&mut self, ctx: &Context) {
        let Ok(path) = std::env::var("CLAUDDY_SHOT") else { return };
        if self.started.elapsed().as_secs_f32() > 8.0 && !self.shot_sent {
            self.shot_sent = true;
            let cb = egui::ScreenshotCallback::new(move |img| {
                let file = std::fs::File::create(&path).expect("shot file");
                let mut enc = png::Encoder::new(file, img.size[0] as u32, img.size[1] as u32);
                enc.set_color(png::ColorType::Rgba);
                enc.set_depth(png::BitDepth::Eight);
                let bytes: Vec<u8> = img.pixels.iter().flat_map(|p| p.to_array()).collect();
                enc.write_header().unwrap().write_image_data(&bytes).unwrap();
                std::process::exit(0);
            });
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(cb));
        }
        ctx.request_repaint_after(Duration::from_millis(200));
    }
}

impl eframe::App for App {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let snap: Snapshot = self.data.lock().map(|s| s.clone()).unwrap_or_default();
        let session = snap.limits.as_ref().and_then(|l| live(&l.session, snap.limits_at));
        let mood = self.pinned.unwrap_or_else(|| mood_across(&snap, session, self.settings.fire_threshold));
        let size = if self.prefs.menu_bar { Size::Expanded } else { self.view.size };
        let screen = (self.view.settings, size);
        // the zoom: the draft's while Settings is open, so a step shows at once
        let zoom_pct = if self.view.settings.is_some() { self.draft.zoom } else { self.prefs.zoom };
        let zoom = zoom_pct as f32 / 100.0;
        if (ctx.zoom_factor() - zoom).abs() > f32::EPSILON {
            ctx.set_zoom_factor(zoom);
        }
        // it takes hold a frame later; then the window, sized in points, is
        // asked for again in the new pixels
        let ppp = ctx.pixels_per_point();
        if self.sized_at != ppp {
            self.sized_at = ppp;
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(self.window.into()));
        }

        // the settled window for this screen
        let want = match screen {
            (Some(tab), _) => settings::size(&snap, tab, &self.view),
            (None, Size::Compact) => {
                let services = crate::screens::connected(&snap);
                let shown = if services.contains(&self.view.provider) { self.view.provider } else { crate::components::logos::Provider::Claude };
                let heads_up = self.prefs.heads_up.min(self.prefs.near_limit) as f64;
                let remind = snap.loaded && crate::components::reminder::state(&snap, shown, session, heads_up).is_some();
                compact::size(services.len(), remind)
            }
            (None, Size::Expanded) => expanded::size(&snap, &self.view, session, self.prefs.heads_up.min(self.prefs.near_limit) as f64),
            (None, Size::PetOnly) => pet_only::size(),
        };
        let card_of = |s: [f32; 2]| Vec2::new(s[0] - 2.0 * MARGIN, s[1] - 2.0 * MARGIN);

        // A new size morphs the card itself: the window (transparent) takes
        // the larger of the two sizes at once, the card eases from the old
        // size to the new one inside it, revealing the new content as it
        // opens, and the window settles to the exact size after. Nothing
        // waits on the OS resizing a window frame by frame.
        if want != self.size {
            let morph = self.size != [0.0, 0.0] && size != Size::PetOnly && self.screen.1 != Size::PetOnly && self.leaving.is_none();
            let fixed = FIXED_CANVAS && !self.prefs.menu_bar && zoom_pct == 100;
            let window = if fixed {
                [CANVAS[0].max(want[0]).max(self.window[0]), CANVAS[1].max(want[1]).max(self.window[1])]
            } else if morph {
                [self.window[0].max(want[0]), self.window[1].max(want[1])]
            } else {
                want
            };
            self.morph = morph.then(|| (self.card_now, Instant::now()));
            self.size = want;
            if window != self.window {
                self.window = window;
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(window.into()));
            }
        }
        // A zoom-only change doesn't alter the logical widget dimensions,
        // but it MUST resize the OS window so text/pets never get cropped.
        if zoom_pct != self.sized_zoom {
            self.sized_zoom = zoom_pct;
            let correct = if FIXED_CANVAS && !self.prefs.menu_bar && zoom_pct == 100 { [CANVAS[0].max(want[0]), CANVAS[1].max(want[1])] } else { want };
            self.window = correct;
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(correct.into()));
        }
        let settled = card_of(want);
        match self.morph {
            Some((from, start)) => {
                let t = motion::progress(start, motion::MORPH_S);
                self.card_now = from + (settled - from) * t;
                if t >= 1.0 {
                    self.morph = None;
                    if !(FIXED_CANVAS && !self.prefs.menu_bar && zoom_pct == 100) && self.window != want {
                        self.window = want;
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(want.into()));
                    }
                } else {
                    ctx.request_repaint();
                }
            }
            None => self.card_now = settled,
        }

        // the new content comes up from part-way, never from nothing (no
        // blink); the panel fades all the way out only on its way to the menu bar
        if screen != self.screen {
            self.screen = screen;
            self.entered = Instant::now();
        }
        let mut fade = 0.55 + 0.45 * motion::progress(self.entered, motion::ENTER_S);
        if let Some(t) = self.leaving {
            fade = 1.0 - motion::progress(t, motion::LEAVE_S);
            if fade <= 0.0 {
                self.leaving = None;
            }
        }
        let origin = ctx.content_rect().min + Vec2::splat(MARGIN);
        let area = egui::Rect::from_min_size(origin, settled);
        let card = egui::Rect::from_min_size(origin, self.card_now);
        // the card stays solid while its content fades: never a see-through moment
        if size != Size::PetOnly && self.leaving.is_none() {
            // the shown service's shade in the expanded panel
            let shown = self.view.provider;
            let tint = if size == Size::Expanded && self.view.settings.is_none() && crate::screens::connected(&snap).contains(&shown) {
                shown.surface()
            } else {
                crate::ui::surface::Tint::Claude
            };
            crate::ui::surface::card_backdrop(ui.painter(), card, tint);
        }
        if fade < 1.0 {
            ui.multiply_opacity(fade);
            ctx.request_repaint();
        }
        if self.morph.is_some() {
            // the content shows only inside the card as it opens or closes
            ui.set_clip_rect(card.expand(1.0));
        }
        // the pet: the one being picked while Settings is open, else the
        // saved one, unless the season's takes over today
        let chosen = if self.view.settings.is_some() { &self.draft.pet } else { &self.prefs.pet };
        let today = chrono::Local::now();
        let (y, m, d) = (chrono::Datelike::year(&today), chrono::Datelike::month(&today), chrono::Datelike::day(&today));
        let theme = if self.view.settings.is_some() { &self.draft.theme } else { &self.prefs.theme };
        self.pet.set_skin(crate::components::skins::active_theme(chosen, theme, y, m, d));
        let codex_skin = crate::components::skins::codex_for_theme(theme, m, d);
        // a new poll: what the pet makes of it
        if snap.seq != self.heard {
            self.heard = snap.seq;
            let before = self.history.counts();
            let (claude_reset, codex_reset) = self.history.sample(&snap, crate::usage::now_ms());
            let after = self.history.counts();
            if claude_reset {
                self.effects.celebrate_reset(crate::components::logos::Provider::Claude, after.claude_weekly > before.claude_weekly);
            }
            if codex_reset {
                self.effects.celebrate_reset(crate::components::logos::Provider::Codex, after.codex_weekly > before.codex_weekly);
            }
            if claude_reset && codex_reset {
                self.effects.team_up();
            }
            let working = [snap.usage.as_ref().is_some_and(|u| u.active), snap.codex.as_ref().is_some_and(|c| c.active)];
            let mut finished = 0;
            for (i, provider) in [crate::components::logos::Provider::Claude, crate::components::logos::Provider::Codex].into_iter().enumerate() {
                if self.was_working[i] && !working[i] {
                    // A verified quota reset celebration is more important
                    // than an ordinary completed task.
                    if ![claude_reset, codex_reset][i] {
                        self.effects.celebrate_finished(provider);
                    }
                    finished += 1;
                }
            }
            if finished == 2 {
                self.effects.team_up();
            }
            self.was_working = working;
            self.listen(&snap, mood, session, size);
        }
        let shown = self.said.as_ref().map_or(0.0, Said::shown);
        if shown <= 0.0 && self.said.as_ref().is_some_and(|s| Instant::now() > s.until) {
            self.said = None;
        } else if let Some(s) = &self.said {
            // through the fades every frame; while it rests, wake when it goes
            let rest = s.until.saturating_duration_since(Instant::now());
            ctx.request_repaint_after(if shown < 1.0 || rest.is_zero() { Duration::ZERO } else { rest });
        }
        let bubble = self.said.as_ref().map(|s| (if size == Size::Compact { s.short.as_str() } else { s.text.as_str() }, shown));
        let f = Frame {
            area,
            card,
            snap: &snap,
            mood,
            session,
            view: &self.view,
            pet: &self.pet,
            codex_skin,
            effects: &self.effects,
            reset_counts: self.history.counts(),
            logos: &self.logos,
            menu_bar: self.prefs.menu_bar,
            heads_up: self.prefs.heads_up.min(self.prefs.near_limit) as f64,
            fire_threshold: self.settings.fire_threshold,
            bubble,
            update: self.updater.state(),
        };

        // the areas that take the pointer; clicks anywhere else fall through
        let mut hot = vec![card];
        let mut actions = match screen {
            (Some(tab), _) => settings::draw(ui, &ctx, &f, tab, &self.draft, &self.prefs, &mut self.code),
            (None, Size::Compact) => compact::draw(ui, &ctx, &f),
            (None, Size::Expanded) => expanded::draw(ui, &ctx, &f, &mut self.code),
            (None, Size::PetOnly) => {
                // the OS's answer where there is one; else what this window sees
                let seen = ctx.input(|i| i.pointer.hover_pos().is_some());
                let over = if cfg!(target_os = "macos") { Some(self.cursor.inside(seen)) } else { None };
                let out = pet_only::draw(ui, &ctx, &f, over);
                hot = out.hot;
                out.actions
            }
        };
        if self.prefs.menu_bar {
            actions.retain(|a| !matches!(a, Action::Resize(_)));
        }
        // The floating panel has a visible resize grip. Drag horizontally
        // or diagonally to scale the entire UI, or click for a 10% increment.
        // Zoom is persisted when the drag finishes; no repeated disk writes.
        let show_grip = self.view.settings.is_none() && size != Size::PetOnly && !self.prefs.menu_bar;
        // just the pet: clicks off the pet fall through to what's behind
        let watching = ((FIXED_CANVAS && !self.prefs.menu_bar) || size == Size::PetOnly) && self.shown && cfg!(target_os = "macos");
        if watching {
            let at = ctx.input(|i| i.viewport().inner_rect).map(|r| r.min.to_vec2()).unwrap_or_default();
            // window points are zoomed; the screen's are not
            let z = ctx.zoom_factor();
            self.cursor.watch(Some(hot.iter().map(|r| r.translate(at) * z).collect()));
        } else {
            self.cursor.watch(None);
        }
        let through = watching && !self.cursor.inside(false);
        if through != self.passthrough {
            self.passthrough = through;
            ctx.send_viewport_cmd(egui::ViewportCommand::MousePassthrough(through));
        }
        // Compute individual animation deadlines before mutating the app.
        let mut companion_next = Duration::from_secs(30);
        if matches!(size, Size::Compact | Size::PetOnly) {
            for provider in crate::screens::connected(&snap) {
                companion_next = companion_next.min(self.pet.next_frame(compact::provider_mood(&f, provider)));
            }
        } else if size == Size::Expanded {
            companion_next = companion_next.min(self.pet.next_frame(compact::provider_mood(&f, crate::screens::shown(&f))));
        }
        self.apply(&ctx, actions);
        self.keyboard_scale(&ctx);
        if show_grip {
            self.resize_grip(ui, &ctx, card);
        }
        if self.effects.animating() {
            ctx.request_repaint_after(Duration::from_millis(60));
        }
        self.menu_bar(&ctx, &snap, session);
        self.shoot(&ctx);
        let mut next = self.pet.next_frame(mood).min(companion_next).min(Duration::from_secs(30));
        // a refused reminder's note goes away on time
        if let Some(left) = crate::components::reminder::error_left(&snap) {
            next = next.min(left);
        }
        ctx.request_repaint_after(next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(active: bool, sleeping: bool) -> Usage {
        let mut u = crate::usage::Reader::new(Some(std::env::temp_dir().join("clauddy-no-such-dir"))).get_usage(&Default::default());
        u.active = active;
        u.sleeping = sleeping;
        u
    }

    #[test]
    fn resize_target_stays_stable_during_drag() {
        let start = 150;
        let drag = Vec2::new(120.0, 80.0);
        // Computing the preview repeatedly must not change the baseline.
        let first = App::drag_zoom(start, drag);
        assert_eq!(first, 245);
        for _ in 0..100 {
            assert_eq!(App::drag_zoom(start, drag), first);
        }
        assert_eq!(App::drag_zoom(150, Vec2::ZERO), 150);
        assert_eq!(App::drag_zoom(150, Vec2::new(-800.0, -800.0)), 75);
        assert_eq!(App::drag_zoom(150, Vec2::new(800.0, 800.0)), 250);
    }

    #[test]
    fn working_wins_then_heat_then_rest() {
        assert_eq!(mood_for(Some(&reading(true, false)), Some(100.0), 90.0), Mood::Working);
        assert_eq!(mood_for(Some(&reading(false, false)), Some(100.0), 90.0), Mood::Tired);
        assert_eq!(mood_for(Some(&reading(false, true)), Some(95.0), 90.0), Mood::Fire);
        assert_eq!(mood_for(Some(&reading(false, true)), Some(40.0), 90.0), Mood::Sleeping);
        assert_eq!(mood_for(Some(&reading(false, false)), None, 90.0), Mood::Idle);
        assert_eq!(mood_for(None, None, 90.0), Mood::Idle);
    }
}
