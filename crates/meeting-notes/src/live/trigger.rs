//! When a live update is worth a model request. Pure rules, independent of time sources.
use super::sources::{pending_chars, LiveSource};
use serde::{Deserialize, Serialize};

/// Minimum new text for a regular interval update.
pub const INTERVAL_CHARS: usize = 200;
/// A continuous contribution of one track at least this long counts as a long contribution.
pub const LONG_TURN_MS: u64 = 90_000;
/// Minimum new text before a long contribution triggers an early update.
pub const LONG_TURN_CHARS: usize = 600;
/// So much new text triggers an update regardless of the interval (about five minutes of speech).
pub const VOLUME_CHARS: usize = 4000;
/// Allowed update intervals.
pub const MIN_INTERVAL_MS: u64 = 60_000;
pub const MAX_INTERVAL_MS: u64 = 15 * 60_000;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Trigger {
    /// The user asked for an update.
    Manual,
    /// Recording and catch-up have finished.
    Final,
    /// A lot of new speech arrived.
    Volume,
    /// One track spoke for a long time and the other track answered.
    LongContribution,
    /// The chosen interval has passed.
    Interval,
}

/// Decides whether pending confirmed text should be summarized now. `since_last_ms` counts
/// from the previous update, or from the start of live notes before the first one.
pub fn decide(
    pending: &[LiveSource],
    since_last_ms: u64,
    interval_ms: u64,
    manual: bool,
    finished: bool,
) -> Option<Trigger> {
    let chars = pending_chars(pending);
    if chars == 0 {
        return None;
    }
    if manual {
        return Some(Trigger::Manual);
    }
    if finished {
        return Some(Trigger::Final);
    }
    if chars >= VOLUME_CHARS {
        return Some(Trigger::Volume);
    }
    if chars >= LONG_TURN_CHARS && long_contribution_ended(pending) {
        return Some(Trigger::LongContribution);
    }
    let interval = interval_ms.clamp(MIN_INTERVAL_MS, MAX_INTERVAL_MS);
    (since_last_ms >= interval && chars >= INTERVAL_CHARS).then_some(Trigger::Interval)
}

/// True when a run of consecutive sections of one track covers the long-turn duration and a
/// later section of the other track follows it.
fn long_contribution_ended(pending: &[LiveSource]) -> bool {
    let mut ordered: Vec<&LiveSource> = pending.iter().collect();
    ordered.sort_by_key(|s| (s.start_ms, s.end_ms));
    let mut run: Option<(&str, u64, u64)> = None;
    for source in ordered {
        match run {
            Some((track, start, end)) if track == source.source => {
                run = Some((track, start, end.max(source.end_ms)));
            }
            Some((_, start, end)) => {
                if end.saturating_sub(start) >= LONG_TURN_MS {
                    return true;
                }
                run = Some((&source.source, source.start_ms, source.end_ms));
            }
            None => run = Some((&source.source, source.start_ms, source.end_ms)),
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(track: &str, start_s: u64, chars: usize) -> LiveSource {
        LiveSource {
            id: format!("{track}{start_s}"),
            key: format!("{track}-{start_s}"),
            chunk: 1,
            source: track.into(),
            start_ms: start_s * 1000,
            end_ms: start_s * 1000 + 30_000,
            text: "x".repeat(chars),
        }
    }
    const MINUTE: u64 = 60_000;

    #[test]
    fn nothing_new_never_triggers_even_manually_or_at_the_end() {
        assert_eq!(decide(&[], 99 * MINUTE, MINUTE, true, true), None);
    }

    #[test]
    fn interval_needs_both_time_and_enough_new_text() {
        let little = [source("system", 0, INTERVAL_CHARS - 1)];
        assert_eq!(decide(&little, 10 * MINUTE, 3 * MINUTE, false, false), None);
        let enough = [source("system", 0, INTERVAL_CHARS)];
        assert_eq!(decide(&enough, 2 * MINUTE, 3 * MINUTE, false, false), None);
        assert_eq!(
            decide(&enough, 3 * MINUTE, 3 * MINUTE, false, false),
            Some(Trigger::Interval)
        );
        // An interval outside the allowed range is clamped instead of polling constantly.
        assert_eq!(decide(&enough, 10_000, 0, false, false), None);
    }

    #[test]
    fn a_long_contribution_followed_by_a_reply_updates_early() {
        let monologue = [
            source("system", 0, 300),
            source("system", 30, 300),
            source("system", 60, 300),
            source("microphone", 95, 50),
        ];
        assert_eq!(
            decide(&monologue, 10_000, 3 * MINUTE, false, false),
            Some(Trigger::LongContribution)
        );
        // Still talking: no reply from the other track yet.
        assert_eq!(
            decide(&monologue[..3], 10_000, 3 * MINUTE, false, false),
            None
        );
        // Alternating short turns are no long contribution.
        let dialogue = [
            source("system", 0, 300),
            source("microphone", 30, 300),
            source("system", 60, 300),
        ];
        assert_eq!(decide(&dialogue, 10_000, 3 * MINUTE, false, false), None);
    }

    #[test]
    fn much_new_text_manual_and_final_updates_do_not_wait_for_the_interval() {
        let lots = [source("system", 0, VOLUME_CHARS)];
        assert_eq!(
            decide(&lots, 0, 3 * MINUTE, false, false),
            Some(Trigger::Volume)
        );
        let some = [source("system", 0, 10)];
        assert_eq!(
            decide(&some, 0, 3 * MINUTE, true, false),
            Some(Trigger::Manual)
        );
        assert_eq!(
            decide(&some, 0, 3 * MINUTE, false, true),
            Some(Trigger::Final)
        );
    }
}
