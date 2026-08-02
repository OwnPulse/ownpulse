// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Canonical dose-status and adherence computation for protocol runs.
//!
//! This is the single source of truth for "what is the status of scheduled
//! day `d` of a protocol line's run" and "what counts toward adherence" —
//! both the web and iOS clients consume the values these produce (via the
//! `doses`, `missed-doses`, and `adherence` endpoints, and the adherence
//! fields on `RunResponse`), so the rules live in exactly these pure
//! functions, with no DB access, fully covered by unit tests. The three SQL
//! paths (`fetch_line_adherence`, `missed_doses`, `list_active_runs`) must
//! implement the identical rule in SQL — see their doc comments.

use chrono::NaiveDate;

/// The adherence status of one scheduled (line, run, day) triple.
///
/// Days that are not scheduled at all (out of range, `schedule_pattern[day]
/// == false`, or inside a pause interval) are not represented here — see
/// [`compute_dose_status`], which returns `None` for those.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoseStatus {
    /// A `protocol_doses` row exists with `status = 'completed'`.
    Completed,
    /// A `protocol_doses` row exists with `status = 'skipped'`.
    Skipped,
    /// Scheduled, in the past (`day_number < today_day`), and no dose row.
    Missed,
    /// Scheduled, today or in the future, and no dose row yet.
    Pending,
}

impl DoseStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            DoseStatus::Completed => "completed",
            DoseStatus::Skipped => "skipped",
            DoseStatus::Missed => "missed",
            DoseStatus::Pending => "pending",
        }
    }
}

/// A `[start_day, end_day)` pause interval, in day-number space relative to
/// a run's `start_date` (so `paused_on`/`resumed_on` dates convert to this
/// via `(date - start_date).num_days()` before calling into this module).
/// `end_day = None` means the run is still paused (no `resumed_on` yet).
#[derive(Debug, Clone, Copy)]
pub struct PauseInterval {
    pub start_day: i32,
    pub end_day: Option<i32>,
}

/// Whether `day_number` falls inside any pause interval.
///
/// A paused day is treated as **not scheduled** everywhere — excluded from
/// `scheduled_so_far`/`completed`/`skipped`/`missed` and therefore from the
/// adherence denominator, in all four implementations (this pure function,
/// and the three SQL paths). Pausing stops the clock: a user who pauses a
/// run for a week does not accrue a week of missed doses for it. This
/// applies uniformly even in the rare case a `protocol_doses` row already
/// exists for a day that a later pause interval covers (logging requires
/// `status = 'active'`, so this should not occur in practice, but "paused
/// days are never scheduled" is simpler to reason about — and to keep
/// consistent across all four call sites — than a partial exception).
pub fn is_paused(day_number: i32, pauses: &[PauseInterval]) -> bool {
    pauses
        .iter()
        .any(|p| day_number >= p.start_day && p.end_day.is_none_or(|end| day_number < end))
}

/// Compute the dose-status of a single (line, day) pair for a run.
///
/// `day_number` is 0-based (day 0 == `run.start_date`). `duration_days` and
/// `pattern` (the line's `schedule_pattern`) together determine whether the
/// day is scheduled at all; `paused` (see [`is_paused`]) overrides that to
/// "not scheduled" regardless of pattern. `existing_status` is the `status`
/// column (`"completed"` or `"skipped"`) of a `protocol_doses` row already
/// logged for this (line, run, day_number), if one exists. `today_day` is
/// `(CURRENT_DATE - start_date)` in days, computed by the caller from
/// Postgres's `CURRENT_DATE` — the database's calendar day, which follows
/// the Postgres session `TimeZone` (UTC by default, but a self-hosted
/// deployment's Postgres may be configured otherwise). Never derive this
/// from the application server's local clock. Clients send `day_number`,
/// not a date; the day-boundary is a database-server concern.
///
/// The dose-log write path tolerates a logged dose landing on
/// `today_day + 1` (a user east of the database's calendar day logging near
/// local midnight, while the database's day hasn't rolled over yet). A day
/// with an existing `protocol_doses` row is always reported as that row's
/// status — `Completed`/`Skipped` — regardless of how it compares to
/// `today_day`. Only a *doseless* day is subject to the past/future split
/// (`Missed`/`Pending`). Note that `Completed`/`Skipped` on `today_day` or
/// `today_day + 1` still do not count toward adherence — see
/// [`adherence_pct`] and [`closed_bound`]; a dose only rolls into adherence
/// once its day has closed.
///
/// Returns `None` when the day is not scheduled — out of the run's
/// duration, past the end of `pattern`, `pattern[day_number] == false`, or
/// paused. "Not scheduled" is deliberately not a [`DoseStatus`] variant:
/// callers (the `doses` and `adherence` endpoints) omit unscheduled days
/// entirely rather than represent them as a dose status.
pub fn compute_dose_status(
    day_number: i32,
    duration_days: i32,
    pattern: &[bool],
    existing_status: Option<&str>,
    today_day: i32,
    paused: bool,
) -> Option<DoseStatus> {
    if paused {
        return None;
    }

    if day_number < 0 || day_number >= duration_days {
        return None;
    }

    let scheduled = pattern
        .get(usize::try_from(day_number).ok()?)
        .copied()
        .unwrap_or(false);
    if !scheduled {
        return None;
    }

    match existing_status {
        Some("completed") => Some(DoseStatus::Completed),
        Some("skipped") => Some(DoseStatus::Skipped),
        // No dose row (None), or an unrecognized status value — fall back
        // to the missed/pending split by day rather than treat unknown
        // status strings as an error; the DB only ever writes "completed"
        // or "skipped", so this arm is a defensive fallback, not a
        // supported input.
        _ => {
            if day_number < today_day {
                Some(DoseStatus::Missed)
            } else {
                Some(DoseStatus::Pending)
            }
        }
    }
}

/// `today_day = (CURRENT_DATE - start_date)`, in days.
pub fn today_day(start_date: NaiveDate, today: NaiveDate) -> i32 {
    // Runs are bounded to `duration_days <= 365` and start dates are never
    // more than a few years in the past/future in practice, so this always
    // fits in i32. No saturating fallback: a value outside i32 range here
    // would indicate a real bug (e.g. a corrupt start_date), and silently
    // substituting a sentinel would mask it rather than surface it.
    i32::try_from((today - start_date).num_days())
        .expect("today_day: date difference exceeds i32 range — should be unreachable")
}

/// Upper bound (inclusive) of the last **closed** day for adherence
/// purposes: `min(today_day - 1, duration_days - 1)`. A day is closed once
/// it is strictly in the past (`day_number < today_day`) — today itself,
/// and any dose logged on the write-path's `today_day + 1` tolerance day,
/// are not yet closed and do not count toward adherence (they still appear
/// in the `/doses` list with their real status). May be negative (the run
/// has no closed day yet); callers pass this directly as a `generate_series`
/// upper bound, which naturally yields zero rows for a negative bound.
pub fn closed_bound(today_day: i32, duration_days: i32) -> i32 {
    (today_day - 1).min(duration_days - 1)
}

/// `completed / (scheduled_so_far - skipped) * 100`, rounded to 1 decimal
/// place.
///
/// A skip is a deliberate decision, not a failure — it is removed from the
/// denominator entirely (never counted against adherence), per the
/// non-judgmental principle: a skip is data, not a failure to be penalized.
/// `completed`, `scheduled_so_far`, and `skipped` must all be bounded to the
/// identical closed-day range (see [`closed_bound`]) — a skip logged today
/// or on the tolerance day must never be subtracted from a denominator that
/// never counted it as scheduled, which would otherwise make negative or
/// over-100% percentages reachable.
///
/// Returns `None` when the denominator (`scheduled_so_far - skipped`) is
/// `<= 0` — nothing scheduled yet, or every closed scheduled day was
/// skipped (0/0 would otherwise be NaN, not a real percentage).
pub fn adherence_pct(completed: i64, scheduled_so_far: i64, skipped: i64) -> Option<f64> {
    let denominator = scheduled_so_far - skipped;
    if denominator <= 0 {
        return None;
    }
    Some((completed as f64 / denominator as f64 * 100.0 * 10.0).round() / 10.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATTERN: [bool; 5] = [true, false, true, true, false];

    #[test]
    fn not_scheduled_when_day_beyond_duration() {
        // duration_days = 3, but pattern is longer — duration wins.
        assert_eq!(compute_dose_status(3, 3, &PATTERN, None, 10, false), None);
    }

    #[test]
    fn not_scheduled_when_day_beyond_pattern_length() {
        // duration_days claims day 5 is valid, but pattern only has 5 entries (0..=4).
        assert_eq!(compute_dose_status(5, 10, &PATTERN, None, 10, false), None);
    }

    #[test]
    fn not_scheduled_when_pattern_false() {
        assert_eq!(compute_dose_status(1, 5, &PATTERN, None, 10, false), None);
    }

    #[test]
    fn not_scheduled_when_day_negative() {
        assert_eq!(compute_dose_status(-1, 5, &PATTERN, None, 10, false), None);
    }

    #[test]
    fn not_scheduled_when_paused_even_if_pattern_true() {
        // Day 0 is scheduled by pattern, but paused overrides to "not scheduled".
        assert_eq!(compute_dose_status(0, 5, &PATTERN, None, 10, true), None);
    }

    #[test]
    fn paused_overrides_even_an_existing_completed_row() {
        // See the doc comment on `is_paused` for why this edge case is
        // resolved as "always not scheduled" rather than a partial exception.
        assert_eq!(
            compute_dose_status(0, 5, &PATTERN, Some("completed"), 10, true),
            None
        );
    }

    #[test]
    fn completed_takes_priority_over_day_math() {
        // Even for a day far in the future, an existing completed row wins.
        assert_eq!(
            compute_dose_status(3, 5, &PATTERN, Some("completed"), 0, false),
            Some(DoseStatus::Completed)
        );
    }

    #[test]
    fn skipped_takes_priority_over_day_math() {
        assert_eq!(
            compute_dose_status(0, 5, &PATTERN, Some("skipped"), 0, false),
            Some(DoseStatus::Skipped)
        );
    }

    #[test]
    fn completed_row_on_today_plus_one_is_reported_as_completed() {
        // A user east of the database's calendar day can log a dose for
        // "tomorrow" from the database's perspective, near local midnight.
        // The write path tolerates day == today_day + 1; the status
        // derivation must not clamp it out of range just because it's
        // nominally "in the future" — an existing dose row always wins.
        // PATTERN[2] == true; today_day=1 means day 2 is "today_day + 1".
        assert_eq!(
            compute_dose_status(2, 5, &PATTERN, Some("completed"), 1, false),
            Some(DoseStatus::Completed)
        );
    }

    #[test]
    fn skipped_row_on_today_plus_one_is_reported_as_skipped() {
        // PATTERN[3] == true; today_day=2 means day 3 is "today_day + 1".
        assert_eq!(
            compute_dose_status(3, 5, &PATTERN, Some("skipped"), 2, false),
            Some(DoseStatus::Skipped)
        );
    }

    #[test]
    fn missed_when_scheduled_past_day_with_no_dose() {
        assert_eq!(
            compute_dose_status(0, 5, &PATTERN, None, 1, false),
            Some(DoseStatus::Missed)
        );
    }

    #[test]
    fn pending_when_scheduled_today_with_no_dose() {
        // today_day == day_number: not yet "in the past".
        assert_eq!(
            compute_dose_status(3, 5, &PATTERN, None, 3, false),
            Some(DoseStatus::Pending)
        );
    }

    #[test]
    fn pending_when_scheduled_future_day_with_no_dose() {
        assert_eq!(
            compute_dose_status(3, 5, &PATTERN, None, 0, false),
            Some(DoseStatus::Pending)
        );
    }

    #[test]
    fn today_day_computation() {
        let start = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 1, 6).unwrap();
        assert_eq!(today_day(start, today), 5);
    }

    #[test]
    fn today_day_zero_on_start_date() {
        let start = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        assert_eq!(today_day(start, start), 0);
    }

    #[test]
    fn today_day_negative_for_future_start() {
        let start = NaiveDate::from_ymd_opt(2026, 1, 10).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        assert_eq!(today_day(start, today), -9);
    }

    #[test]
    fn is_paused_inside_closed_interval() {
        let pauses = [PauseInterval {
            start_day: 2,
            end_day: Some(5),
        }];
        assert!(!is_paused(1, &pauses));
        assert!(is_paused(2, &pauses));
        assert!(is_paused(4, &pauses));
        assert!(!is_paused(5, &pauses));
    }

    #[test]
    fn is_paused_inside_open_interval_still_paused() {
        // No resumed_on yet: still paused, unbounded.
        let pauses = [PauseInterval {
            start_day: 2,
            end_day: None,
        }];
        assert!(is_paused(2, &pauses));
        assert!(is_paused(1000, &pauses));
        assert!(!is_paused(1, &pauses));
    }

    #[test]
    fn is_paused_across_multiple_intervals() {
        let pauses = [
            PauseInterval {
                start_day: 2,
                end_day: Some(4),
            },
            PauseInterval {
                start_day: 8,
                end_day: Some(10),
            },
        ];
        assert!(is_paused(3, &pauses));
        assert!(is_paused(9, &pauses));
        assert!(!is_paused(5, &pauses));
        assert!(!is_paused(11, &pauses));
    }

    #[test]
    fn closed_bound_basic() {
        // today_day=5, duration=10: days 0-4 are closed.
        assert_eq!(closed_bound(5, 10), 4);
    }

    #[test]
    fn closed_bound_negative_before_run_has_a_closed_day() {
        // A run started today (today_day=0) has no closed day yet.
        assert_eq!(closed_bound(0, 10), -1);
    }

    #[test]
    fn closed_bound_capped_by_duration() {
        // today_day far beyond duration: bound caps at duration_days - 1.
        assert_eq!(closed_bound(100, 10), 9);
    }

    #[test]
    fn adherence_pct_null_when_nothing_scheduled() {
        assert_eq!(adherence_pct(0, 0, 0), None);
    }

    #[test]
    fn adherence_pct_null_when_every_closed_day_skipped() {
        // scheduled=3, skipped=3 -> denominator 0, not 0/0 NaN.
        assert_eq!(adherence_pct(0, 3, 3), None);
    }

    #[test]
    fn adherence_pct_excludes_skips_from_denominator() {
        // 5 scheduled, 1 skipped -> denominator 4; 2 completed -> 50%.
        assert_eq!(adherence_pct(2, 5, 1), Some(50.0));
    }

    #[test]
    fn adherence_pct_rounds_to_one_decimal() {
        // 1/3 * 100 = 33.333...  -> 33.3
        assert_eq!(adherence_pct(1, 3, 0), Some(33.3));
    }

    #[test]
    fn adherence_pct_full_adherence() {
        assert_eq!(adherence_pct(4, 4, 0), Some(100.0));
    }
}
