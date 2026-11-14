//! Approximate API-equivalent token value, calculated from local usage logs.
//! This is NOT a subscription charge, paid credit balance, or provider invoice.
//! Rates: https://platform.claude.com/docs/en/about-claude/pricing
//! and https://developers.openai.com/api/docs/pricing (USD per million, Nov 2026).
//! Long-context and special service-tier surcharges cannot always be inferred
//! from local logs; unknown models are excluded rather than assigned fake rates.

#[derive(Clone, Copy)]
struct Rates {
    input: f64,
    cached: f64,
    write_short: f64,
    write_long: f64,
    output: f64,
}
impl Rates {
    fn value(self, input: u64, cached: u64, write_short: u64, write_long: u64, output: u64) -> f64 {
        (input as f64 * self.input
            + cached as f64 * self.cached
            + write_short as f64 * self.write_short
            + write_long as f64 * self.write_long
            + output as f64 * self.output)
            / 1_000_000.0
    }
}

fn claude_rates(model: &str) -> Option<Rates> {
    let (input, cached, short, long, output) = if model.contains("opus-5-5") {
        (4.0, 0.20, 5.0, 8.0, 20.0)
    } else if model.contains("sonnet-5-5") {
        (2.0, 0.10, 2.5, 4.0, 10.0)
    } else if model.contains("fable-5-2") {
        (8.0, 0.20, 10.0, 16.0, 40.0)
    } else if model.contains("opus-4-7") || model.contains("opus-4-6") || model.contains("opus-4-5") {
        (5.0, 0.50, 6.25, 10.0, 25.0)
    } else if model.contains("sonnet-4-6") || model.contains("sonnet-4-5") {
        (3.0, 0.30, 3.75, 6.0, 15.0)
    } else if model.contains("haiku-4-5") {
        (1.0, 0.10, 1.25, 2.0, 5.0)
    } else {
        return None;
    };
    Some(Rates { input, cached, write_short: short, write_long: long, output })
}

/// Claude cache writes have separate 5m and 1h billing rates.
pub fn claude(model: &str, input: u64, output: u64, cache_read: u64, cache_write_5m: u64, cache_write_1h: u64) -> Option<f64> {
    Some(claude_rates(model)?.value(input, cache_read, cache_write_5m, cache_write_1h, output))
}

fn codex_rates(model: &str) -> Option<Rates> {
    let (input, cached, short, long, output) = match model {
        "gpt-6-astra" => (10.0, 1.0, 12.50, 25.0, 50.0),
        "gpt-6.1-sol" => (2.0, 0.10, 2.50, 5.0, 10.0),
        "gpt-6-sol" => (2.0, 0.20, 2.50, 5.0, 10.0),
        "gpt-6.1-terra" => (1.0, 0.05, 1.25, 2.50, 5.0),
        "gpt-6-luna" => (0.10, 0.01, 0.125, 0.25, 0.50),
        "gpt-5.6-sol" | "gpt-5.6" => (4.0, 0.40, 5.0, 10.0, 20.0),
        "gpt-5.5" => (5.0, 0.50, 5.0, 5.0, 30.0),
        "gpt-5.3-codex" | "gpt-5.2-codex" => (1.75, 0.175, 1.75, 1.75, 14.0),
        _ => return None,
    };
    Some(Rates { input, cached, write_short: short, write_long: long, output })
}

/// Codex total input already includes cache-read tokens and (when present)
/// cache-write tokens. Do not count those in full-price input as well.
pub fn codex(model: &str, input: u64, output: u64, cached: u64, cache_write: u64) -> Option<f64> {
    let rates = codex_rates(model)?;
    let uncached = input.saturating_sub(cached).saturating_sub(cache_write);
    Some(rates.value(uncached, cached, cache_write, 0, output))
}

/// A dollar sign with a partial-coverage marker, never a fake credit balance.
pub fn amount(usd: f64, unpriced_tokens: u64) -> String {
    if unpriced_tokens > 0 { format!("≥${usd:.2}") } else { format!("${usd:.2}") }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cached_input_is_discounted_and_not_double_counted() {
        // 1M inputs, 800K cached, 100K output, Sol 6.1 rates:
        // 200K * $2 + 800K * $0.10 + 100K * $10 = $1.48.
        assert_eq!(codex("gpt-6.1-sol", 1_000_000, 100_000, 800_000, 0), Some(1.48));
    }
    #[test]
    fn one_hour_claude_cache_writes_use_the_higher_rate() {
        assert_eq!(claude("claude-opus-5-5", 0, 0, 0, 0, 1_000_000), Some(8.0));
        assert_eq!(claude("claude-opus-5-5", 0, 0, 0, 1_000_000, 0), Some(5.0));
    }
    #[test]
    fn unknown_models_are_not_valued_as_zero() {
        assert_eq!(codex("codex-auto-review", 10_000, 50, 0, 0), None);
        assert_eq!(claude("unknown", 10_000, 0, 0, 0, 0), None);
        assert_eq!(amount(12.0, 1), "≥$12.00");
    }
}
