// Local, conservative reset-event observation. It does not invent a provider's
// allowance or claim the number of future quota resets.
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use crate::components::meter::live;
use crate::poller::Snapshot;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Counts {
    pub claude_5h: u32,
    pub claude_weekly: u32,
    pub codex_5h: u32,
    pub codex_weekly: u32,
}

impl Counts {
    pub fn pair(self, codex: bool) -> (u32, u32) {
        if codex { (self.codex_5h, self.codex_weekly) } else { (self.claude_5h, self.claude_weekly) }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Seen {
    used: f64,
    reset_at: i64,
    count: u32,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct History {
    windows: BTreeMap<String, Seen>,
}

impl History {
    fn path() -> PathBuf {
        crate::auth::default_data_dir().join("reset-history.json")
    }

    pub fn load() -> Self {
        fs::read(Self::path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    fn persist(&self) {
        if let Ok(data) = serde_json::to_vec_pretty(self) {
            let path = Self::path();
            if let Some(dir) = path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            #[cfg(unix)]
            {
                use std::io::Write;
                use std::os::unix::fs::OpenOptionsExt;
                if let Ok(mut f) = fs::OpenOptions::new().create(true).truncate(true).write(true).mode(0o600).open(path) {
                    let _ = f.write_all(&data);
                }
            }
            #[cfg(not(unix))]
            {
                let _ = fs::write(path, data);
            }
        }
    }

    /// Only mark a reset when a previously observed window has elapsed, the
    /// new deadline has moved forward significantly and usage fell. A rolling
    /// window merely extending while tokens are used must never count.
    pub fn observe(&mut self, key: &str, used: f64, reset_at: i64, now: i64, duration: i64) -> bool {
        if !used.is_finite() || !(0.0..=100.0).contains(&used) || reset_at <= now || reset_at > now + duration + 3_600_000 {
            return false;
        }
        let seen = self.windows.entry(key.to_string()).or_insert_with(|| Seen { used, reset_at, count: 0 });
        let reset = now >= seen.reset_at && reset_at > seen.reset_at + duration / 2 && used + 2.0 < seen.used;
        if reset {
            seen.count = seen.count.saturating_add(1);
        }
        seen.used = used;
        seen.reset_at = reset_at;
        reset
    }

    pub fn counts(&self) -> Counts {
        let c = |name: &str| self.windows.get(name).map_or(0, |w| w.count);
        Counts { claude_5h: c("claude-5h"), claude_weekly: c("claude-weekly"), codex_5h: c("codex-5h"), codex_weekly: c("codex-weekly") }
    }

    /// Returns services that experienced a verified session or weekly reset.
    pub fn sample(&mut self, snap: &Snapshot, now: i64) -> (bool, bool) {
        const H: i64 = 3_600_000;
        let mut claude = false;
        let mut codex = false;
        if snap.connected
            && let Some(l) = &snap.limits
        {
            if let Some((pct, Some(ms))) = live(&l.session, snap.limits_at) {
                claude |= self.observe("claude-5h", pct, now + ms, now, 5 * H);
            }
            if let Some((pct, Some(ms))) = live(&l.week, snap.limits_at) {
                claude |= self.observe("claude-weekly", pct, now + ms, now, 7 * 24 * H);
            }
        }
        if let Some(c) = &snap.codex
            && c.limits_at.is_some_and(|at| (0..=15 * 60_000).contains(&(now - at)))
        {
            if !c.weekly_only
                && let Some(w) = &c.session
                && !w.expired
                && let (Some(pct), Some(ms)) = (w.pct, w.reset_ms)
            {
                codex |= self.observe("codex-5h", pct, now + ms, now, 5 * H);
            }
            let week = if c.weekly_only { c.session.as_ref() } else { c.weekly.as_ref() };
            if let Some(w) = week
                && !w.expired
                && let (Some(pct), Some(ms)) = (w.pct, w.reset_ms)
            {
                codex |= self.observe("codex-weekly", pct, now + ms, now, 7 * 24 * H);
            }
        }
        self.persist();
        (claude, codex)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_five_hour_and_weekly_independently() {
        let now = 50 * 3_600_000_i64;
        let mut h = History::default();
        assert!(!h.observe("codex-5h", 97.0, now + 60_000, now, 5 * 3_600_000));
        assert!(!h.observe("codex-weekly", 99.0, now + 60_000, now, 7 * 24 * 3_600_000));
        assert!(h.observe("codex-5h", 6.0, now + 5 * 3_600_000 + 120_000, now + 120_000, 5 * 3_600_000));
        assert!(h.observe("codex-weekly", 2.0, now + 7 * 24 * 3_600_000 + 120_000, now + 120_000, 7 * 24 * 3_600_000));
        assert_eq!(h.counts().pair(true), (1, 1));
        assert_eq!(h.counts().pair(false), (0, 0));
    }
    #[test]
    fn no_counter_on_rolling_deadline_or_no_usage_drop() {
        let mut h = History::default();
        let n = 100_000_000;
        assert!(!h.observe("claude-5h", 20.0, n + 2_000_000, n, 18_000_000));
        assert!(!h.observe("claude-5h", 33.0, n + 8_000_000, n + 1_000_000, 18_000_000));
        assert!(!h.observe("claude-5h", 40.0, n + 18_000_000, n + 8_100_000, 18_000_000));
        assert_eq!(h.counts().claude_5h, 0);
    }
}
