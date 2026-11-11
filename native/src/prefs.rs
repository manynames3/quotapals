// The settings people change, kept in ~/.claude-usage-monitor/config.json,
// shared with older versions. Saving rewrites only these keys, so everything
// else in it (budgets, notes, keys only older versions read) is left as it was.

use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prefs {
    /// gentle notifications on the way to the limit
    pub alerts: bool,
    pub heads_up: i32,
    pub near_limit: i32,
    /// the session % at which the pet catches fire
    pub fire: i32,
    /// floating on the desktop, or in the menu bar
    pub menu_bar: bool,
    /// Keep a native menu-bar icon visible while the widget still floats.
    pub menu_bar_alongside: bool,
    /// Automatic calendar theme or a user-selected outfit for both pets.
    pub theme: String,
    /// Codex tracked alongside Claude
    pub codex: bool,
    /// Cursor tracked alongside Claude
    pub cursor: bool,
    /// Opt-in automatic continuation for usage-paused Claude Code sessions.
    pub resume_claude: bool,
    /// Opt-in automatic continuation for usage-paused Codex sessions.
    pub resume_codex: bool,
    /// the widget's scale, 100-200 %
    pub zoom: i32,
    /// the speech bubble (on unless turned off)
    pub talk: bool,
    /// the chiptune blips (opt-in)
    pub sound: bool,
    /// a mute in force until then (epoch ms)
    pub muted_until: i64,
    /// the pet picked in Settings (a seasonal one may take over on its days)
    pub pet: String,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            alerts: true,
            heads_up: 80,
            near_limit: 95,
            fire: 90,
            menu_bar: false,
            menu_bar_alongside: false,
            theme: "auto".into(),
            codex: false,
            cursor: false,
            resume_claude: false,
            resume_codex: false,
            zoom: 100,
            talk: true,
            sound: false,
            muted_until: 0,
            pet: crate::components::skins::DEFAULT.into(),
        }
    }
}

fn path() -> std::path::PathBuf {
    crate::auth::default_data_dir().join("config.json")
}

fn read() -> Value {
    std::fs::read(path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_else(|| json!({}))
}

impl Prefs {
    pub fn from_value(v: &Value) -> Self {
        let d = Prefs::default();
        let pct = |x: Option<i64>, fallback: i32| x.map(|n| n as i32).unwrap_or(fallback);
        let t = v["alertThresholds"].as_array();
        Prefs {
            alerts: v["alerts"].as_bool().unwrap_or(d.alerts),
            heads_up: pct(t.and_then(|a| a.first()).and_then(Value::as_i64), d.heads_up),
            near_limit: pct(t.and_then(|a| a.get(1)).and_then(Value::as_i64), d.near_limit),
            fire: pct(v["fireThreshold"].as_i64(), d.fire),
            menu_bar: v["mode"].as_str() == Some("menubar"),
            menu_bar_alongside: v["mode"].as_str() == Some("both"),
            theme: v["theme"].as_str().unwrap_or("auto").to_string(),
            codex: v.pointer("/codex/enabled").and_then(Value::as_bool).unwrap_or(false),
            cursor: v.pointer("/cursor/enabled").and_then(Value::as_bool).unwrap_or(false),
            resume_claude: v.pointer("/autoContinue/claude").and_then(Value::as_bool).unwrap_or(false),
            resume_codex: v.pointer("/autoContinue/codex").and_then(Value::as_bool).unwrap_or(false),
            talk: v["talk"].as_bool() != Some(false),
            pet: v["pet"].as_str().unwrap_or(crate::components::skins::DEFAULT).to_string(),
            sound: v["sound"].as_bool().unwrap_or(false),
            muted_until: v["soundMutedUntil"].as_f64().unwrap_or(0.0) as i64,
            zoom: v["zoom"].as_f64().map(|z| z.round() as i32).unwrap_or(d.zoom),
        }
        .clamped()
    }

    pub fn load() -> Self {
        Prefs::from_value(&read())
    }

    /// Thresholds in range: 1-100, fire below 100 (100 is maxed out).
    pub fn clamped(mut self) -> Self {
        self.heads_up = self.heads_up.clamp(1, 100);
        self.near_limit = self.near_limit.clamp(1, 100);
        self.fire = self.fire.clamp(1, 99);
        self.zoom = self.zoom.clamp(100, 200);
        if !matches!(self.theme.as_str(), "auto" | "classic" | "christmas") {
            self.theme = "auto".into();
        }
        self
    }

    /// Write these keys into `v`, leaving the rest alone.
    pub fn merge_into(&self, v: &mut Value) {
        if !v.is_object() {
            *v = json!({});
        }
        v["alerts"] = json!(self.alerts);
        v["alertThresholds"] = json!([self.heads_up, self.near_limit]);
        v["fireThreshold"] = json!(self.fire);
        v["zoom"] = json!(self.zoom);
        v["talk"] = json!(self.talk);
        v["sound"] = json!(self.sound);
        v["soundMutedUntil"] = json!(self.muted_until);
        v["pet"] = json!(self.pet);
        v["theme"] = json!(self.theme);
        v["mode"] = json!(if self.menu_bar {
            "menubar"
        } else if self.menu_bar_alongside {
            "both"
        } else {
            "floating"
        });
        if !v["autoContinue"].is_object() {
            v["autoContinue"] = json!({});
        }
        v["autoContinue"]["claude"] = json!(self.resume_claude);
        v["autoContinue"]["codex"] = json!(self.resume_codex);
        for (key, on) in [("codex", self.codex), ("cursor", self.cursor)] {
            if !v[key].is_object() {
                v[key] = json!({});
            }
            v[key]["enabled"] = json!(on);
        }
    }

    pub fn save(&self) {
        let mut v = read();
        self.merge_into(&mut v);
        if let Ok(body) = serde_json::to_string_pretty(&v) {
            let _ = std::fs::create_dir_all(crate::auth::default_data_dir());
            let _ = std::fs::write(path(), body);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_electron_keys_and_falls_back_to_defaults() {
        let p = Prefs::from_value(&json!({"alerts": false, "alertThresholds": [70, 90], "fireThreshold": 85}));
        assert_eq!(
            p,
            Prefs {
                alerts: false,
                heads_up: 70,
                near_limit: 90,
                fire: 85,
                menu_bar: false,
                menu_bar_alongside: false,
                theme: "auto".into(),
                codex: false,
                cursor: false,
                resume_claude: false,
                resume_codex: false,
                zoom: 100,
                talk: true,
                sound: false,
                muted_until: 0,
                pet: "clauddy".into(),
            }
        );
        assert_eq!(Prefs::from_value(&json!({"pet": "wizard"})).pet, "wizard");
        assert!(Prefs::from_value(&json!({"codex": {"enabled": true}})).codex);
        assert!(Prefs::from_value(&json!({"cursor": {"enabled": true}})).cursor);
        assert!(Prefs::from_value(&json!({"mode": "menubar"})).menu_bar);
        assert!(Prefs::from_value(&json!({"mode": "both"})).menu_bar_alongside);
        assert_eq!(Prefs::from_value(&json!({"theme": "christmas"})).theme, "christmas");
        assert_eq!(Prefs::from_value(&json!({"theme": "invalid"})).theme, "auto");
        assert_eq!(Prefs::from_value(&json!({"zoom": 150})).zoom, 150);
        assert_eq!(Prefs::from_value(&json!({"zoom": 900})).zoom, 200);
        assert_eq!(Prefs::from_value(&json!({})), Prefs::default());
    }

    #[test]
    fn saving_keeps_every_other_key() {
        // a pet this version doesn't have is kept as it was, not reset
        let mut v = json!({"plan": "max5x", "pet": "pumpkin", "fireThreshold": 90});
        Prefs { fire: 75, ..Prefs::from_value(&v) }.merge_into(&mut v);
        assert_eq!(v["fireThreshold"], 75);
        assert_eq!(v["pet"], "pumpkin");
        assert_eq!(v["plan"], "max5x");
    }

    #[test]
    fn auto_continue_is_opt_in_and_persists_independently() {
        let mut prefs = Prefs::default();
        assert!(!prefs.resume_claude && !prefs.resume_codex);
        prefs.resume_codex = true;
        let mut config = json!({"preserve": {"other": 9}});
        prefs.merge_into(&mut config);
        let loaded = Prefs::from_value(&config);
        assert!(!loaded.resume_claude && loaded.resume_codex);
        assert_eq!(config["preserve"]["other"], 9);
    }

    #[test]
    fn thresholds_stay_in_range() {
        let p = Prefs { heads_up: 0, near_limit: 140, fire: 100, ..Prefs::default() }.clamped();
        assert_eq!((p.heads_up, p.near_limit, p.fire), (1, 100, 99));
    }
}
