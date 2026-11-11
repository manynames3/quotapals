// How numbers and times read on the widget.

/// "13m", "2h 13m", past a day "6d 16h".
pub fn countdown(ms: i64) -> String {
    let m = ms.max(0) / 60_000;
    let (d, h, m) = (m / 1440, m % 1440 / 60, m % 60);
    match (d, h) {
        (0, 0) => format!("{m}m"),
        (0, _) => format!("{h}h {m}m"),
        _ => format!("{d}d {h}h"),
    }
}

/// Now, as the widget reads clocks. Snapshots pin it so their text never moves.
fn now() -> chrono::DateTime<chrono::FixedOffset> {
    #[cfg(test)]
    if let Some(t) = FROZEN.get() {
        return *t;
    }
    chrono::Local::now().fixed_offset()
}

/// An epoch-ms instant on the local clock.
pub fn local(ms: i64) -> chrono::DateTime<chrono::Local> {
    chrono::DateTime::from_timestamp_millis(ms).unwrap_or_default().with_timezone(&chrono::Local)
}

#[cfg(test)]
pub static FROZEN: std::sync::OnceLock<chrono::DateTime<chrono::FixedOffset>> = std::sync::OnceLock::new();

/// The wall-clock time a reset lands on: "4:40 PM", "Mon 11:37 AM" past a
/// day, "Nov 12" a week or more out.
pub fn clock(ms: i64) -> String {
    let at = now() + chrono::Duration::milliseconds(ms.max(0));
    if ms >= 6 * 86_400_000 {
        at.format("%b %-d").to_string()
    } else if ms >= 86_400_000 {
        at.format("%a %-I:%M %p").to_string()
    } else {
        at.format("%-I:%M %p").to_string()
    }
}

/// "2h 13m (12:50 PM)": the countdown and the clock time it lands on.
pub fn reset_in(ms: i64) -> String {
    format!("{} ({})", countdown(ms), clock(ms))
}

/// Approximate number of five-hour session-reset boundaries before the
/// weekly reset. A session window may move or not recur without activity,
/// so this is a projection—not guaranteed usable credits or future quota.
pub fn projected_five_hour_resets(session_ms: Option<i64>, week_ms: Option<i64>) -> Option<u32> {
    const FIVE_HOURS: i64 = 5 * 60 * 60 * 1000;
    let (session, week) = (session_ms?, week_ms?);
    if session <= 0 || week <= 0 {
        return None;
    }
    if session >= week {
        return Some(0);
    }
    Some((1 + (week - session - 1) / FIVE_HOURS).min(40) as u32)
}

/// "37.8M", "1.12B".
pub fn tokens(n: u64) -> String {
    match n {
        n if n >= 1_000_000_000 => format!("{:.2}B", n as f64 / 1e9),
        n if n >= 1_000_000 => format!("{:.1}M", n as f64 / 1e6),
        n if n >= 1_000 => format!("{:.1}k", n as f64 / 1e3),
        n => n.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countdowns_and_token_counts_read_short() {
        assert_eq!(countdown(13 * 60_000 + 59_000), "13m");
        assert_eq!(countdown((2 * 60 + 13) * 60_000), "2h 13m");
        assert_eq!(countdown((25 * 60) * 60_000), "1d 1h");
        assert!(reset_in(2 * 3_600_000).starts_with("2h 0m ("));
        assert_eq!(tokens(37_800_000), "37.8M");
        assert_eq!(tokens(1_120_000_000), "1.12B");
        assert_eq!(tokens(950), "950");
        let h = 3_600_000;
        assert_eq!(projected_five_hour_resets(Some(h), Some(13 * h)), Some(3));
        assert_eq!(projected_five_hour_resets(Some(4 * h), Some(3 * h)), Some(0));
        assert_eq!(projected_five_hour_resets(None, Some(3 * h)), None);
    }
}
