// Optional, local, best-effort resumption of conversations stopped by quota.
// Inspired by CodexReset (MIT, BOYSO); no inaccessible GUI automation.
// This worker is inert until a person turns on a provider's Resume switch.
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::poller::{Shared, Snapshot};
use crate::prefs::Prefs;
use crate::usage::now_ms;

const HOUR: i64 = 3_600_000;
const DAY: i64 = 24 * HOUR;
const MAX_FAILURE_AGE: i64 = 7 * DAY;
const FRESH_READING_AGE: i64 = 10 * 60_000;
const WAKE_SECS: u64 = 50;
const LIMIT_PER_POLL: usize = 3;

/// The UI can report why a switch is enabled but not resuming anything.
/// These status labels contain no session IDs or private conversation data.
static CLAUDE_STATUS: AtomicU8 = AtomicU8::new(0);
static CODEX_STATUS: AtomicU8 = AtomicU8::new(0);
const OFF: u8 = 1;
const DELEGATED: u8 = 2;
const WAITING_DATA: u8 = 3;
const NO_PAUSED: u8 = 4;
const WAITING_RESET: u8 = 5;
const SENT: u8 = 6;
const LAUNCH_FAILED: u8 = 7;
const PERMISSION_BLOCKED: u8 = 8;
fn status_cell(provider: &str) -> &'static AtomicU8 {
    if provider == "codex" { &CODEX_STATUS } else { &CLAUDE_STATUS }
}
fn set_status(provider: &str, value: u8) {
    status_cell(provider).store(value, Ordering::Relaxed);
}
pub fn status_label(provider: &str) -> &'static str {
    match status_cell(provider).load(Ordering::Relaxed) {
        OFF => "Off",
        DELEGATED => "CodexReset active; TokenRunway paused",
        WAITING_DATA => "Waiting for fresh quota reading",
        NO_PAUSED => "No eligible usage-paused chats",
        WAITING_RESET => "Waiting for confirmed reset",
        SENT => "Resume sent; completion unverified",
        LAUNCH_FAILED => "CLI failed; see local log",
        PERMISSION_BLOCKED => "Resume was blocked by permissions; see local log",
        _ => "Checking local sessions...",
    }
}

#[derive(Clone, Copy, Debug)]
struct Quota {
    used: f64,
    next_reset_ms: i64,
    duration_ms: i64,
}

/// An individual, most-recent turn that ended in an explicit usage-limit error.
#[derive(Clone, Debug)]
struct Paused {
    id: String,
    cwd: PathBuf,
    failed_ms: i64,
    /// Exact reset timestamp reported by the quota error, if available.
    reset_at_ms: Option<i64>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct State {
    /// Full IDs stay in a file in the user's home directory, never in logs.
    issued: BTreeSet<String>,
}

fn data_path() -> PathBuf {
    crate::auth::default_data_dir().join("auto-continue-state.json")
}

fn read_state() -> State {
    fs::read(data_path()).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

fn save_state(state: &State) {
    let path = data_path();
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Ok(raw) = serde_json::to_vec_pretty(state) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            if let Ok(mut f) = fs::OpenOptions::new().write(true).truncate(true).create(true).mode(0o600).open(path) {
                let _ = f.write_all(&raw);
            }
        }
        #[cfg(not(unix))]
        {
            let _ = fs::write(path, raw);
        }
    }
}

fn note(message: &str) {
    let path = crate::auth::default_data_dir().join("auto-continue.log");
    let _ = fs::create_dir_all(crate::auth::default_data_dir());
    let line = format!("{} {message}\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).mode(0o600).open(path) {
            let _ = file.write_all(line.as_bytes());
        }
    }
    #[cfg(not(unix))]
    {
        use std::fs::OpenOptions;
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = file.write_all(line.as_bytes());
        }
    }
}

/// A calendar deadline alone is never sufficient: both percentages must
/// actually be below 100% in a recent provider reading.
fn recovered_after(windows: &[Quota], failed_ms: i64, now: i64) -> bool {
    !windows.is_empty()
        && windows.iter().all(|w| w.used.is_finite() && (0.0..100.0).contains(&w.used) && w.next_reset_ms > now)
        && windows.iter().any(|w| {
            let start = w.next_reset_ms - w.duration_ms;
            start > failed_ms + 10_000 && start <= now
        })
}

fn claude_windows(snap: &Snapshot, now: i64) -> Vec<Quota> {
    let (Some(limits), Some(at)) = (&snap.limits, snap.limits_at) else { return vec![] };
    if !snap.connected || at.elapsed().as_millis() > FRESH_READING_AGE as u128 {
        return vec![];
    }
    let shift = at.elapsed().as_millis() as i64;
    let mut result = Vec::new();
    for (w, duration) in [(&limits.session, 5 * HOUR), (&limits.week, 7 * DAY)] {
        if let Some(reset) = w.reset_ms
            && reset > shift
        {
            result.push(Quota { used: w.pct, next_reset_ms: now + reset - shift, duration_ms: duration });
        }
    }
    result
}

fn codex_windows(snap: &Snapshot, now: i64) -> Vec<Quota> {
    let Some(c) = snap.codex.as_ref() else { return vec![] };
    if c.limits_at.is_none_or(|at| now - at > FRESH_READING_AGE || now < at - 60_000) {
        return vec![];
    }
    let mut result = Vec::new();
    for (w, duration) in [(c.session.as_ref(), if c.weekly_only { 7 * DAY } else { 5 * HOUR }), (c.weekly.as_ref(), 7 * DAY)] {
        if let Some(w) = w
            && !w.expired
            && let (Some(pct), Some(reset)) = (w.pct, w.reset_ms)
            && reset > 0
        {
            result.push(Quota { used: pct, next_reset_ms: now + reset, duration_ms: duration });
        }
    }
    result
}

/// Only known UUIDs can reach a CLI command. Never invoke a shell.
fn valid_id(id: &str) -> bool {
    id.len() == 36 && id.bytes().enumerate().all(|(i, ch)| if [8, 13, 18, 23].contains(&i) { ch == b'-' } else { ch.is_ascii_hexdigit() })
}

fn codex_home() -> PathBuf {
    std::env::var_os("CODEX_HOME").map(PathBuf::from).unwrap_or_else(|| std::env::home_dir().unwrap_or_default().join(".codex"))
}

/// Mirrors CodexReset's confirmed usageLimitExceeded detection and excludes
/// any failed thread with a newer turn. The SQLite database is read-only.
fn codex_paused(now: i64) -> Vec<Paused> {
    let home = codex_home();
    let Ok(db) = Connection::open_with_flags(home.join("thread_history_1.sqlite"), OpenFlags::SQLITE_OPEN_READ_ONLY) else { return vec![] };
    let Ok(mut query) = db.prepare(
        "SELECT t.thread_id, t.started_at FROM thread_turns t \
         WHERE t.status = 'failed' AND t.error_json LIKE '%usageLimitExceeded%' \
         AND NOT EXISTS (SELECT 1 FROM thread_turns newer \
         WHERE newer.thread_id = t.thread_id AND newer.turn_id > t.turn_id) \
         ORDER BY t.turn_id DESC LIMIT 60",
    ) else {
        return vec![];
    };
    let Ok(rows) = query.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?.unwrap_or(0)))) else { return vec![] };

    let state = Connection::open_with_flags(home.join("state_5.sqlite"), OpenFlags::SQLITE_OPEN_READ_ONLY).ok();
    rows.flatten()
        .filter_map(|(id, secs)| {
            let failed_ms = secs.saturating_mul(1000);
            if !valid_id(&id) || !(0..MAX_FAILURE_AGE).contains(&(now - failed_ms)) {
                return None;
            }
            let (cwd, source): (String, String) =
                state.as_ref().and_then(|db| db.query_row("SELECT cwd, source FROM threads WHERE id=?1", [&id], |r| Ok((r.get(0)?, r.get(1)?))).ok())?;
            if source.trim_start().starts_with("{\"subagent\"") {
                return None;
            }
            let cwd = PathBuf::from(cwd);
            if !cwd.is_dir() {
                return None;
            }
            Some(Paused { id, cwd, failed_ms, reset_at_ms: None })
        })
        .collect()
}

fn quota_error(s: &str) -> bool {
    let s = s.to_ascii_lowercase();
    ["usage limit", "usage_limit", "rate limit", "rate_limit", "limit reached", "hit your limit", "reached your limit"].iter().any(|word| s.contains(word))
}

fn claude_error_reset_ms(v: &Value) -> Option<i64> {
    let reset = v
        .pointer("/apiErrorParams/rate_limit_info/resetsAt")
        .and_then(Value::as_i64)
        .or_else(|| v.pointer("/rateLimitInfo/resetsAt").and_then(Value::as_i64))?;
    Some(if reset < 100_000_000_000 { reset.saturating_mul(1000) } else { reset })
}

/// A reset timestamp plus a *later* fresh, below-limit account reading
/// avoids guessing from a missing 5-hour countdown or a stale 0%.
fn claude_ready_after_error(session: &Paused, read_at_ms: i64, session_used: f64, week_used: f64, now: i64) -> bool {
    let Some(reset) = session.reset_at_ms else { return false };
    // Give Claude Desktop its own built-in auto-continue a short head start.
    reset > session.failed_ms
        && reset.saturating_add(90_000) <= now
        && read_at_ms > reset
        && [session_used, week_used].iter().all(|p| p.is_finite() && (0.0..100.0).contains(p))
}

fn claude_ready(snap: &Snapshot, session: &Paused, now: i64) -> bool {
    let (Some(limits), Some(at)) = (&snap.limits, snap.limits_at) else { return false };
    let age = at.elapsed().as_millis() as i64;
    if !snap.connected || age > FRESH_READING_AGE {
        return false;
    }
    claude_ready_after_error(session, now - age, limits.session.pct, limits.week.pct, now)
}

fn claude_error_entry(v: &Value) -> bool {
    let kind = v["type"].as_str().unwrap_or("");
    let subtype = v["subtype"].as_str().unwrap_or("");
    // Recent Claude Desktop and Claude Code write structured 429 quota errors.
    if v["isApiErrorMessage"].as_bool() == Some(true)
        && (v["apiError"].as_str() == Some("usage_limit_reached") || (v["apiErrorStatus"].as_i64() == Some(429) && v["error"].as_str() == Some("rate_limit")))
    {
        return true;
    }
    if kind == "rate_limit_event" {
        return matches!(v.pointer("/rateLimitInfo/status").and_then(Value::as_str), Some("rejected" | "limited"));
    }
    if kind == "system" && matches!(subtype, "api_error" | "rate_limit" | "usage_limit") {
        return quota_error(&v.to_string());
    }
    kind == "assistant" && v["isApiErrorMessage"].as_bool() == Some(true) && quota_error(&v["message"].to_string())
}

/// Only a *terminal*, explicit API-limit failure qualifies; a user asking
/// about rate limits or a completed Claude response must never qualify.
fn claude_paused_file(path: &Path, now: i64) -> Option<Paused> {
    let id = path.file_stem()?.to_str()?.to_string();
    if !valid_id(&id) {
        return None;
    }
    let meta = fs::metadata(path).ok()?;
    let failed_ms = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as i64;
    if !(0..MAX_FAILURE_AGE).contains(&(now - failed_ms)) {
        return None;
    }
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    if len > 160_000 {
        file.seek(SeekFrom::Start(len - 160_000)).ok()?;
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    // A tail may start halfway through a UTF-8 character.
    let buf = String::from_utf8_lossy(&bytes);
    let mut quota_error_found = false;
    let mut error_at = None;
    let mut reset_at_ms = None;
    for raw in buf.lines().rev().take(180) {
        let Ok(v) = serde_json::from_str::<Value>(raw) else { continue };
        let kind = v["type"].as_str().unwrap_or("");
        if !quota_error_found {
            if !matches!(kind, "assistant" | "user" | "system" | "rate_limit_event") {
                continue;
            }
            if claude_error_entry(&v) {
                // A sidechain may still be managed by its parent session.
                if v["isSidechain"].as_bool() == Some(true) {
                    return None;
                }
                let ts = v["timestamp"].as_str().and_then(crate::usage::parse_date)?;
                if !(0..MAX_FAILURE_AGE).contains(&(now - ts)) {
                    return None;
                }
                quota_error_found = true;
                error_at = Some(ts);
                reset_at_ms = claude_error_reset_ms(&v);
            } else if kind == "user" || kind == "assistant" {
                // The latest completed turn was not paused by the limit.
                return None;
            } else {
                continue;
            }
        }
        // Rate-limit events often don't include cwd. The preceding transcript
        // message has it; looking backward is safe after confirming the last
        // meaningful event is an API-limit failure.
        if let Some(cwd) = v["cwd"].as_str().map(PathBuf::from)
            && cwd.is_dir()
        {
            return Some(Paused { id, cwd, failed_ms: error_at?, reset_at_ms });
        }
    }
    None
}

fn claude_paused(now: i64) -> Vec<Paused> {
    let root = crate::usage::default_claude_dir().join("projects");
    let mut found = Vec::new();
    if let Ok(projects) = fs::read_dir(root) {
        for project in projects.flatten() {
            if let Ok(files) = fs::read_dir(project.path()) {
                for entry in files.flatten() {
                    if entry.path().extension().is_some_and(|ext| ext == "jsonl")
                        && let Some(p) = claude_paused_file(&entry.path(), now)
                    {
                        found.push(p);
                    }
                }
            }
        }
    }
    found.sort_by_key(|p| std::cmp::Reverse(p.failed_ms));
    found.truncate(60);
    found
}

fn find_cli(name: &str) -> Option<PathBuf> {
    let home = std::env::home_dir().unwrap_or_default();
    for path in [home.join(".local/bin").join(name), PathBuf::from("/opt/homebrew/bin").join(name), PathBuf::from("/usr/local/bin").join(name)] {
        if path.is_file() {
            return Some(path);
        }
    }
    std::env::var_os("PATH").and_then(|path| std::env::split_paths(&path).map(|dir| dir.join(name)).find(|path| path.is_file()))
}

fn other_resumer_running() -> bool {
    #[cfg(target_os = "macos")]
    {
        let Ok(out) = Command::new("/usr/bin/pgrep").args(["-f", "^/Applications/CodexReset.app/Contents/MacOS/CodexReset$"]).output() else { return false };
        if !out.status.success() {
            return false;
        }
        // If CodexReset also has automatic resumption enabled, don't race it.
        // When its toggle is off, it is safe to use TokenRunway instead.
        let enabled = Command::new("/usr/bin/defaults")
            .args(["read", "com.codexreset.CodexReset", "autoContinue"])
            .output()
            .ok()
            .and_then(|v| String::from_utf8(v.stdout).ok());
        enabled.is_none_or(|value| value.trim() != "0")
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

fn dispatch(provider: &str, session: &Paused) -> bool {
    let Some(bin) = find_cli(if provider == "claude" { "claude" } else { "codex" }) else {
        note(&format!("{provider}: CLI not found; cannot resume"));
        return false;
    };
    let mut cmd = Command::new(bin);
    cmd.current_dir(&session.cwd).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    if provider == "claude" {
        // Auto mode applies Claude Code's action classifier to unattended tool
        // requests. Never bypass permissions. If something still requires
        // manual approval, --permission-prompts none denies rather than hangs.
        cmd.args(["-p", "--resume", &session.id, "--max-turns", "20",
            "--permission-mode", "auto", "--permission-prompts", "none",
            "Continue the unfinished work after the usage reset. Inspect the current project state and retry only incomplete background tasks; do not repeat completed changes."]);
    } else {
        cmd.args(["exec", "--sandbox", "workspace-write", "-C"]);
        cmd.arg(&session.cwd);
        cmd.args(["resume", &session.id, "Continue where you left off."]);
    }
    // Preserve private CLI output locally to distinguish permission, quota,
    // and transport failures. Never echo conversation contents into app logs.
    let run_dir = crate::auth::default_data_dir().join("auto-continue-runs");
    let _ = fs::create_dir_all(&run_dir);
    let log_path = run_dir.join(format!("{provider}-{}-{}.log", session.id, session.failed_ms));
    let output = {
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            fs::OpenOptions::new().create(true).truncate(true).write(true).mode(0o600).open(&log_path).ok()
        }
        #[cfg(not(unix))]
        {
            File::create(&log_path).ok()
        }
    };
    if let Some(file) = output
        && let Ok(stderr) = file.try_clone()
    {
        cmd.stdout(Stdio::from(file)).stderr(Stdio::from(stderr));
    }
    match cmd.spawn() {
        Ok(mut child) => {
            // A successful spawn is not proof the model completed; never
            // retry automatically after dispatch to avoid duplicate turns.
            let provider = provider.to_string();
            std::thread::spawn(move || match child.wait() {
                Ok(status) if status.success() => {
                    let blocked = fs::read_to_string(&log_path).ok().is_some_and(|text| {
                        let text = text.to_ascii_lowercase();
                        text.contains("permission mode blocked")
                            || text.contains("blocked by permissions")
                            || text.contains("permission request was denied")
                            || text.contains("permissions denied")
                    });
                    if blocked {
                        set_status(&provider, PERMISSION_BLOCKED);
                        note(&format!("{provider}: CLI exited successfully but its report says permissions blocked an action; inspect {}", log_path.display()));
                    } else {
                        note(&format!("{provider}: resume CLI exited successfully (original task completion unverified)"));
                    }
                }
                Ok(status) => {
                    set_status(&provider, LAUNCH_FAILED);
                    note(&format!("{provider}: resume CLI exited with status {:?}; inspect {}", status.code(), log_path.display()));
                }
                Err(e) => note(&format!("{provider}: could not observe resume CLI exit: {e}")),
            });
            true
        }
        Err(e) => {
            note(&format!("{provider}: could not start CLI ({e})"));
            false
        }
    }
}

/// A completely separate worker from the UI and quota poller. Off by default.
pub fn spawn(shared: Shared) {
    let _ = std::thread::Builder::new().name("auto-continue".into()).spawn(move || {
        let mut state = read_state();
        let mut last_seq = 0;
        let mut warned_codexreset = false;
        loop {
            std::thread::sleep(Duration::from_secs(WAKE_SECS));
            let prefs = Prefs::load();
            if !prefs.resume_claude {
                set_status("claude", OFF);
            }
            if !prefs.resume_codex {
                set_status("codex", OFF);
            }
            if !prefs.resume_claude && !prefs.resume_codex {
                continue;
            }
            let Ok(snap) = shared.lock().map(|s| s.clone()) else { continue };
            if snap.seq == last_seq {
                continue;
            }
            last_seq = snap.seq;
            let now = now_ms();
            let codexreset_collision = prefs.resume_codex && other_resumer_running();
            if codexreset_collision && !warned_codexreset {
                note("Codex auto-continue deferred: CodexReset is running with auto-continue enabled. Disable its toggle or quit it.");
            }
            warned_codexreset = codexreset_collision;
            if codexreset_collision {
                set_status("codex", DELEGATED);
            }
            let services = [
                ("claude", prefs.resume_claude, claude_windows(&snap, now)),
                ("codex", prefs.resume_codex && !codexreset_collision, codex_windows(&snap, now)),
            ];
            for (provider, enabled, windows) in services {
                if !enabled {
                    continue;
                }
                // Claude's 5h reset timestamp can be absent even when the
                // account API has a fresh 0%-used reading; its failed turn
                // carries the exact provider reset timestamp instead.
                if windows.is_empty() && provider != "claude" {
                    set_status(provider, WAITING_DATA);
                    continue;
                }
                let failures = if provider == "claude" { claude_paused(now) } else { codex_paused(now) };
                if failures.is_empty() {
                    set_status(provider, NO_PAUSED);
                    continue;
                }
                let mut sent = 0;
                let mut eligible = 0;
                let mut previously_issued = false;
                for session in failures {
                    if sent >= LIMIT_PER_POLL {
                        break;
                    }
                    let ready = if provider == "claude" { claude_ready(&snap, &session, now) } else { recovered_after(&windows, session.failed_ms, now) };
                    if !ready {
                        continue;
                    }
                    let key = format!("{provider}:{}:{}", session.id, session.failed_ms);
                    if state.issued.contains(&key) {
                        previously_issued = true;
                        continue;
                    }
                    eligible += 1;
                    if dispatch(provider, &session) {
                        state.issued.insert(key);
                        while state.issued.len() > 300 {
                            if let Some(oldest) = state.issued.iter().next().cloned() {
                                state.issued.remove(&oldest);
                            }
                        }
                        save_state(&state);
                        note(&format!("{provider}: dispatched one previously rate-limited session (unverified completion)"));
                        sent += 1;
                    }
                }
                if sent > 0 {
                    set_status(provider, SENT);
                } else if eligible > 0 {
                    set_status(provider, LAUNCH_FAILED);
                } else if previously_issued {
                    // Preserve a worker-reported failure instead of replacing
                    // it with "Waiting for confirmed reset" on the next poll.
                    if status_cell(provider).load(Ordering::Relaxed) != PERMISSION_BLOCKED {
                        set_status(provider, SENT);
                    }
                } else {
                    set_status(provider, WAITING_RESET);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_requires_fresh_below_limit_reading_from_later_window() {
        let n = 10 * HOUR;
        let w = Quota { used: 18.0, next_reset_ms: n + 4 * HOUR, duration_ms: 5 * HOUR };
        assert!(recovered_after(&[w], n - 2 * HOUR, n));
        assert!(!recovered_after(&[w], n - 10 * 60_000, n)); // same window
        assert!(!recovered_after(&[Quota { used: 100.0, ..w }], n - 2 * HOUR, n));
        assert!(!recovered_after(&[], n - 2 * HOUR, n));
        assert!(!recovered_after(&[Quota { next_reset_ms: n - 1, ..w }], n - 2 * HOUR, n));
        assert!(!recovered_after(&[w, Quota { used: 100.0, ..w }], n - 2 * HOUR, n));
    }

    #[test]
    fn only_explicit_api_quota_errors_count() {
        assert!(claude_error_entry(&serde_json::json!({"type":"assistant","isApiErrorMessage":true,"message":"Usage limit reached"})));
        assert!(!claude_error_entry(&serde_json::json!({"type":"assistant","message":"Here's how rate limits work"})));
        assert!(claude_error_entry(&serde_json::json!({"type":"rate_limit_event","rateLimitInfo":{"status":"rejected"}})));
        assert!(!claude_error_entry(&serde_json::json!({"type":"rate_limit_event","rateLimitInfo":{"status":"allowed"}})));
    }

    #[test]
    fn claude_only_resumes_terminal_quota_error_not_normal_conversation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("11111111-2222-3333-4444-555555555555.jsonl");
        let cwd = dir.path().to_str().unwrap();
        let ts = chrono::Utc::now().to_rfc3339();
        let entries = [
            serde_json::json!({"type":"user","cwd":cwd,"message":"Please work on this project","timestamp":ts}),
            serde_json::json!({"type":"rate_limit_event","timestamp":ts,"rateLimitInfo":{"status":"rejected"}}),
        ];
        std::fs::write(&path, entries.iter().map(|e| format!("{e}\n")).collect::<String>()).unwrap();
        let now = now_ms();
        let p = claude_paused_file(&path, now).expect("explicit rejected quota event");
        assert_eq!(p.cwd, dir.path());
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(format!("{}\n", serde_json::json!({"type":"assistant","message":"All done"})).as_bytes())
            .unwrap();
        assert!(claude_paused_file(&path, now_ms()).is_none());
    }

    #[test]
    fn structured_429_quota_error_and_reported_reset() {
        let v = serde_json::json!({"type":"assistant","isApiErrorMessage":true,
            "apiErrorStatus":429,"apiError":"usage_limit_reached","error":"rate_limit",
            "apiErrorParams":{"rate_limit_info":{"resetsAt":1_794_627_600}}});
        assert!(claude_error_entry(&v));
        assert_eq!(claude_error_reset_ms(&v), Some(1_794_627_600_000));
        assert!(!claude_error_entry(&serde_json::json!({"type":"assistant",
            "isApiErrorMessage":true,"error":"authentication_failed"})));
    }

    #[test]
    fn claude_requires_confirmed_reset_and_fresh_below_limit_account_reading() {
        let n = 1_794_628_000_000i64;
        let s = Paused { id: "11111111-2222-3333-4444-555555555555".into(), cwd: std::env::temp_dir(), failed_ms: n - 900_000, reset_at_ms: Some(n - 120_000) };
        assert!(claude_ready_after_error(&s, n - 60_000, 0.0, 84.0, n));
        assert!(!claude_ready_after_error(&s, n - 180_000, 0.0, 84.0, n));
        assert!(!claude_ready_after_error(&s, n - 60_000, 0.0, 100.0, n));
        assert!(!claude_ready_after_error(&Paused { reset_at_ms: None, ..s }, n - 60_000, 0.0, 84.0, n));
    }

    #[test]
    fn invalid_session_ids_cannot_reach_the_cli() {
        assert!(valid_id("11111111-2222-3333-4444-555555555555"));
        assert!(!valid_id("--dangerously-bypass-approvals-and-sandbox"));
        assert!(!valid_id("../../../whatever"));
    }
}
