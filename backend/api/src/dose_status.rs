// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Canonical dose-status computation for protocol run adherence.
//!
//! This is the single source of truth for "what is the status of scheduled
//! day `d` of a protocol line's run" — both the web and iOS clients consume
//! the values this produces (via the `doses`, `missed-doses`, and
//! `adherence` endpoints), so the rule lives in exactly one pure function
//! with no DB access, fully covered by unit tests.

use chrono::NaiveDate;

/// The adherence status of one scheduled (line, run, day) triple.
///
/// Days that are not scheduled at all (out of range, or
/// `schedule_pattern[day] == false`) are not represented here — see
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

/// Compute the dose-status of a single (line, day) pair for a run.
///
/// `day_number` is 0-based (day 0 == `run.start_date`). `duration_days` and
/// `pattern` (the line's `schedule_pattern`) together determine whether the
/// day is scheduled at all. `existing_status` is the `status` column
/// (`"completed"` or `"skipped"`) of a `protocol_doses` row already logged
/// for this (line, run, day_number), if one exists. `today_day` is
/// `(CURRENT_DATE - start_date)` in days, computed by the caller from
/// Postgres's `CURRENT_DATE` (one clock, server-side, UTC) — never derive
/// it from the application server's local clock. Clients send
/// `day_number`, not a date; the day-boundary is a server-side concern.
///
/// The dose-log write path tolerates a logged dose landing on
/// `today_day + 1` (a user east of UTC logging near local midnight, while
/// the server's UTC day hasn't rolled over yet). A day with an existing
/// `protocol_doses` row is always reported as that row's status —
/// `Completed`/`Skipped` — regardless of how it compares to `today_day`.
/// Only a *doseless* day is subject to the past/future split
/// (`Missed`/`Pending`).
///
/// Returns `None` when the day is not scheduled — out of the run's
/// duration, past the end of `pattern`, or `pattern[day_number] == false`.
/// "Not scheduled" is deliberately not a [`DoseStatus`] variant: callers
/// (the `doses` and `adherence` endpoints) omit unscheduled days entirely
/// rather than represent them as a dose status.
pub fn compute_dose_status(
    day_number: i32,
    duration_days: i32,
    pattern: &[bool],
    existing_status: Option<&str>,
    today_day: i32,
) -> Option<DoseStatus> {
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

/// `today_day = (CURRENT_DATE - start_date)`, computed on the server in UTC.
pub fn today_day(start_date: NaiveDate, today: NaiveDate) -> i32 {
    // Runs are bounded to `duration_days <= 365` and start dates are never
    // more than a few years in the past/future in practice, so this always
    // fits in i32; saturate defensively rather than panic.
    i32::try_from((today - start_date).num_days()).unwrap_or(if today > start_date {
        i32::MAX
    } else {
        i32::MIN
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATTERN: [bool; 5] = [true, false, true, true, false];

    #[test]
    fn not_scheduled_when_day_beyond_duration() {
        // duration_days = 3, but pattern is longer — duration wins.
        assert_eq!(compute_dose_status(3, 3, &PATTERN, None, 10), None);
    }

    #[test]
    fn not_scheduled_when_day_beyond_pattern_length() {
        // duration_days claims day 5 is valid, but pattern only has 5 entries (0..=4).
        assert_eq!(compute_dose_status(5, 10, &PATTERN, None, 10), None);
    }

    #[test]
    fn not_scheduled_when_pattern_false() {
        assert_eq!(compute_dose_status(1, 5, &PATTERN, None, 10), None);
    }

    #[test]
    fn not_scheduled_when_day_negative() {
        assert_eq!(compute_dose_status(-1, 5, &PATTERN, None, 10), None);
    }

    #[test]
    fn completed_takes_priority_over_day_math() {
        // Even for a day far in the future, an existing completed row wins.
        assert_eq!(
            compute_dose_status(3, 5, &PATTERN, Some("completed"), 0),
            Some(DoseStatus::Completed)
        );
    }

    #[test]
    fn skipped_takes_priority_over_day_math() {
        assert_eq!(
            compute_dose_status(0, 5, &PATTERN, Some("skipped"), 0),
            Some(DoseStatus::Skipped)
        );
    }

    #[test]
    fn completed_row_on_today_plus_one_is_reported_as_completed() {
        // A user east of UTC can log a dose for "tomorrow" from the
        // server's UTC perspective, near local midnight. The write path
        // tolerates day == today_day + 1; the status derivation must not
        // clamp it out of range just because it's nominally "in the
        // future" — an existing dose row always wins.
        // PATTERN[2] == true; today_day=1 means day 2 is "today_day + 1".
        assert_eq!(
            compute_dose_status(2, 5, &PATTERN, Some("completed"), 1),
            Some(DoseStatus::Completed)
        );
    }

    #[test]
    fn skipped_row_on_today_plus_one_is_reported_as_skipped() {
        // PATTERN[3] == true; today_day=2 means day 3 is "today_day + 1".
        assert_eq!(
            compute_dose_status(3, 5, &PATTERN, Some("skipped"), 2),
            Some(DoseStatus::Skipped)
        );
    }

    #[test]
    fn missed_when_scheduled_past_day_with_no_dose() {
        assert_eq!(
            compute_dose_status(0, 5, &PATTERN, None, 1),
            Some(DoseStatus::Missed)
        );
    }

    #[test]
    fn pending_when_scheduled_today_with_no_dose() {
        // today_day == day_number: not yet "in the past".
        assert_eq!(
            compute_dose_status(3, 5, &PATTERN, None, 3),
            Some(DoseStatus::Pending)
        );
    }

    #[test]
    fn pending_when_scheduled_future_day_with_no_dose() {
        assert_eq!(
            compute_dose_status(3, 5, &PATTERN, None, 0),
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
}
