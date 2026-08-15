// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Google Calendar API HTTP client and meeting-aggregation logic.
//!
//! **Privacy boundary**: event titles, descriptions, and locations never
//! cross into this process at all. The Calendar API request sends
//! `fields=items(start(dateTime),end(dateTime),attendees(self,responseStatus)),nextPageToken`
//! and `eventTypes=default` (server-side filtering out-of-office/focus-time/
//! working-location entries with zero extra fields read), so Google's
//! response is restricted to exactly what this module needs — content
//! fields aren't merely ignored after being fetched, they're never sent by
//! the server in the first place. `attendees` is requested only to check
//! whether the calendar owner declined a meeting (see [`aggregate_into`])
//! and exists solely for the duration of that check — it is never stored,
//! logged, or returned from this module. Each page of events is folded into
//! a per-day aggregate and dropped immediately
//! ([`GoogleCalendarClient::fetch_and_aggregate`]), so no full event list —
//! content or otherwise — is ever held in memory. `calendar_days` stores
//! meeting *counts and minutes only* (see
//! `docs/decisions/0011-explore-and-observer-polls.md` and CLAUDE.md's
//! "Cooperative data boundary"); there is no code path in this module through
//! which meeting content could reach the database, logs, or an error
//! message.
//!
//! **Day bucketing**: days are UTC calendar days. An evening meeting for a
//! user west of UTC may be bucketed onto the next UTC day — this is a known
//! limitation, not a bug; user-timezone-aware bucketing is a tracked
//! follow-up (see `docs/architecture/api.md`).

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;

/// Build a short, sanitized error message for a non-2xx Google Calendar
/// response. Only the status and context are logged — never the response
/// body. Google's error envelopes can echo back request context, and a
/// misconfigured or proxied endpoint could return arbitrary content, so the
/// body is dropped entirely rather than logged at any level (same rule
/// `integrations::oura`/`garmin` follow).
fn sanitized_upstream_error(context: &str, status: reqwest::StatusCode) -> String {
    tracing::debug!(%status, context, "Google Calendar API returned a non-2xx response");
    format!("{context}: Google Calendar API returned HTTP {status}")
}

/// Typed fetch outcome so callers can distinguish "access token rejected"
/// (worth a refresh + one retry) from any other failure.
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("unauthorized (401) — access token rejected by Google")]
    Unauthorized,
    #[error("{0}")]
    Other(String),
}

/// An event's start or end instant. Only `dateTime` is requested/deserialized
/// — an all-day event (which Google represents with a `date` field instead)
/// simply has this as `None`, which is all `aggregate_into` needs to exclude
/// it as "not a timed meeting". There is no need to read or store the `date`
/// value itself.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventTime {
    pub date_time: Option<DateTime<Utc>>,
}

/// A single attendee entry, requested only to determine whether the
/// calendar owner declined this meeting. **Transient**: this struct exists
/// only for the lifetime of parsing one page of events in
/// [`GoogleCalendarClient::fetch_and_aggregate`] — it is never stored,
/// logged, or returned from this module. No other attendee field (email,
/// display name, etc.) is ever requested or deserialized.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Attendee {
    #[serde(rename = "self")]
    is_self: Option<bool>,
    response_status: Option<String>,
}

/// A single calendar event's timing plus (transiently) attendance status.
/// Google's API response also includes `summary`, `description`, and
/// `location` — none of those are ever requested or deserialized here.
#[derive(Debug, Deserialize)]
pub struct CalendarEvent {
    pub start: Option<EventTime>,
    pub end: Option<EventTime>,
    #[serde(default)]
    attendees: Option<Vec<Attendee>>,
}

/// True if the calendar owner (the `self` attendee) declined this event.
/// Declined meetings don't count toward the day's aggregate — see
/// `aggregate_into` and `userdocs/docs/integrations.md`.
fn self_declined(event: &CalendarEvent) -> bool {
    event.attendees.as_ref().is_some_and(|attendees| {
        attendees
            .iter()
            .any(|a| a.is_self.unwrap_or(false) && a.response_status.as_deref() == Some("declined"))
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CalendarEventsResponse {
    items: Option<Vec<CalendarEvent>>,
    next_page_token: Option<String>,
}

/// Client for the Google Calendar v3 API. `api_base_url` is overridable for
/// WireMock testing; defaults to `https://www.googleapis.com`.
pub struct GoogleCalendarClient {
    http: reqwest::Client,
    pub api_base_url: String,
}

impl GoogleCalendarClient {
    pub fn new(http: reqwest::Client, api_base_url: Option<String>) -> Self {
        Self {
            http,
            api_base_url: api_base_url.unwrap_or_else(|| "https://www.googleapis.com".to_string()),
        }
    }

    /// Fetch events between `time_min` and `time_max` from the user's
    /// primary Google Calendar, folding each page directly into a per-day
    /// meeting aggregate rather than collecting every event into memory.
    ///
    /// `singleEvents=true` expands recurring events into individual
    /// instances (required for correct per-day aggregation); `eventTypes=default`
    /// excludes out-of-office/focus-time/working-location entries server-side.
    /// The `fields` param restricts the response to exactly what this module
    /// deserializes — see the module docs' privacy boundary section. Each
    /// page's `Vec<CalendarEvent>` is folded and dropped before the next
    /// page is requested, so memory use is bounded by one page regardless of
    /// how many events a calendar/window holds.
    pub async fn fetch_and_aggregate(
        &self,
        access_token: &str,
        time_min: DateTime<Utc>,
        time_max: DateTime<Utc>,
    ) -> Result<BTreeMap<NaiveDate, DayAggregate>, FetchError> {
        let mut days: BTreeMap<NaiveDate, DayAggregate> = BTreeMap::new();
        let mut page_token: Option<String> = None;

        loop {
            let url = format!("{}/calendar/v3/calendars/primary/events", self.api_base_url);
            let mut request = self.http.get(&url).bearer_auth(access_token).query(&[
                ("timeMin", time_min.to_rfc3339()),
                ("timeMax", time_max.to_rfc3339()),
                ("singleEvents", "true".to_string()),
                ("orderBy", "startTime".to_string()),
                ("eventTypes", "default".to_string()),
                (
                    "fields",
                    "items(start(dateTime),end(dateTime),attendees(self,responseStatus)),nextPageToken"
                        .to_string(),
                ),
            ]);

            if let Some(ref token) = page_token {
                request = request.query(&[("pageToken", token.as_str())]);
            }

            let response = request
                .send()
                .await
                .map_err(|e| FetchError::Other(format!("calendar events request failed: {e}")))?;

            let status = response.status();
            if !status.is_success() {
                if status == reqwest::StatusCode::UNAUTHORIZED {
                    tracing::debug!(%status, "Google Calendar API returned a non-2xx response");
                    return Err(FetchError::Unauthorized);
                }
                return Err(FetchError::Other(sanitized_upstream_error(
                    "fetch events",
                    status,
                )));
            }

            let page: CalendarEventsResponse = response.json().await.map_err(|e| {
                FetchError::Other(format!("failed to parse calendar events response: {e}"))
            })?;

            if let Some(items) = page.items {
                aggregate_into(&mut days, &items);
            } // `items` (and any per-event data, including `attendees`) drops here, before the next page is fetched.

            match page.next_page_token {
                Some(token) => page_token = Some(token),
                None => break,
            }
        }

        Ok(days)
    }
}

/// Per-day meeting aggregate — counts and total minutes only. Never carries
/// titles, attendees, or descriptions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayAggregate {
    pub meeting_count: i32,
    pub meeting_minutes: i32,
}

/// Aggregate a list of events into per-day meeting counts/minutes.
///
/// Only *timed* events (both `start.dateTime` and `end.dateTime` present)
/// count as meetings — all-day entries (birthdays, holidays, out-of-office
/// blocks) have no `dateTime` and aren't "meetings" in the sense this table
/// tracks, so they're skipped entirely. Zero- or negative-duration events
/// (malformed upstream data) are also skipped rather than recorded as a
/// meeting with nonsensical duration. Events the calendar owner declined
/// (`attendees` contains a `self` entry with `responseStatus == "declined"`)
/// are skipped too — a declined meeting isn't time the user actually spent
/// in a meeting.
///
/// An event is bucketed by the UTC calendar date of its *start* time — a
/// meeting spanning midnight is not split across two days, it counts in
/// full on the day it started. This keeps aggregation a single pass with no
/// cross-day double counting.
pub fn aggregate_by_day(events: &[CalendarEvent]) -> BTreeMap<NaiveDate, DayAggregate> {
    let mut days: BTreeMap<NaiveDate, DayAggregate> = BTreeMap::new();
    aggregate_into(&mut days, events);
    days
}

/// Fold `events` into an existing per-day aggregate map. Used by
/// [`aggregate_by_day`] (a fresh map, for callers/tests that already have a
/// full event list) and by [`GoogleCalendarClient::fetch_and_aggregate`]
/// (folded page-by-page, so no full event list is ever materialized).
fn aggregate_into(days: &mut BTreeMap<NaiveDate, DayAggregate>, events: &[CalendarEvent]) {
    for event in events {
        if self_declined(event) {
            continue;
        }

        let Some(start) = event.start.as_ref() else {
            continue;
        };
        let Some(end) = event.end.as_ref() else {
            continue;
        };
        let (Some(start_dt), Some(end_dt)) = (start.date_time, end.date_time) else {
            continue; // all-day event, or malformed — not a timed meeting
        };
        if end_dt <= start_dt {
            continue; // zero/negative duration — skip rather than record garbage
        }

        let date = start_dt.date_naive();
        let minutes = (end_dt - start_dt).num_minutes() as i32;

        let entry = days.entry(date).or_insert(DayAggregate {
            meeting_count: 0,
            meeting_minutes: 0,
        });
        entry.meeting_count += 1;
        entry.meeting_minutes += minutes;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timed_event(start: &str, end: &str) -> CalendarEvent {
        CalendarEvent {
            start: Some(EventTime {
                date_time: Some(start.parse().unwrap()),
            }),
            end: Some(EventTime {
                date_time: Some(end.parse().unwrap()),
            }),
            attendees: None,
        }
    }

    fn declined_event(start: &str, end: &str) -> CalendarEvent {
        CalendarEvent {
            attendees: Some(vec![Attendee {
                is_self: Some(true),
                response_status: Some("declined".to_string()),
            }]),
            ..timed_event(start, end)
        }
    }

    fn all_day_event() -> CalendarEvent {
        CalendarEvent {
            start: Some(EventTime { date_time: None }),
            end: Some(EventTime { date_time: None }),
            attendees: None,
        }
    }

    #[test]
    fn aggregates_multiple_events_same_day() {
        let events = vec![
            timed_event("2026-06-01T09:00:00Z", "2026-06-01T09:30:00Z"),
            timed_event("2026-06-01T10:00:00Z", "2026-06-01T11:00:00Z"),
        ];
        let days = aggregate_by_day(&events);
        assert_eq!(days.len(), 1);
        let date = "2026-06-01".parse().unwrap();
        assert_eq!(
            days[&date],
            DayAggregate {
                meeting_count: 2,
                meeting_minutes: 90
            }
        );
    }

    #[test]
    fn separates_events_across_days() {
        let events = vec![
            timed_event("2026-06-01T09:00:00Z", "2026-06-01T09:30:00Z"),
            timed_event("2026-06-02T09:00:00Z", "2026-06-02T09:30:00Z"),
        ];
        let days = aggregate_by_day(&events);
        assert_eq!(days.len(), 2);
    }

    #[test]
    fn all_day_events_are_not_meetings() {
        let events = vec![all_day_event()];
        let days = aggregate_by_day(&events);
        assert!(
            days.is_empty(),
            "all-day entries must not be counted as meetings"
        );
    }

    #[test]
    fn zero_duration_events_are_skipped() {
        let events = vec![timed_event("2026-06-01T09:00:00Z", "2026-06-01T09:00:00Z")];
        let days = aggregate_by_day(&events);
        assert!(days.is_empty());
    }

    #[test]
    fn missing_start_or_end_is_skipped() {
        let events = vec![
            CalendarEvent {
                start: None,
                end: Some(EventTime {
                    date_time: Some("2026-06-01T09:00:00Z".parse().unwrap()),
                }),
                attendees: None,
            },
            CalendarEvent {
                start: Some(EventTime {
                    date_time: Some("2026-06-01T09:00:00Z".parse().unwrap()),
                }),
                end: None,
                attendees: None,
            },
        ];
        let days = aggregate_by_day(&events);
        assert!(days.is_empty());
    }

    #[test]
    fn meeting_spanning_midnight_counts_on_start_day() {
        let events = vec![timed_event("2026-06-01T23:30:00Z", "2026-06-02T00:30:00Z")];
        let days = aggregate_by_day(&events);
        assert_eq!(days.len(), 1);
        let date = "2026-06-01".parse().unwrap();
        assert_eq!(days[&date].meeting_minutes, 60);
    }

    #[test]
    fn declined_events_are_not_meetings() {
        let events = vec![declined_event(
            "2026-06-01T09:00:00Z",
            "2026-06-01T09:30:00Z",
        )];
        let days = aggregate_by_day(&events);
        assert!(
            days.is_empty(),
            "a meeting the owner declined must not count"
        );
    }

    #[test]
    fn other_attendee_declining_does_not_affect_count() {
        let mut event = timed_event("2026-06-01T09:00:00Z", "2026-06-01T09:30:00Z");
        event.attendees = Some(vec![Attendee {
            is_self: Some(false),
            response_status: Some("declined".to_string()),
        }]);
        let days = aggregate_by_day(&[event]);
        assert_eq!(
            days.len(),
            1,
            "another attendee declining must not exclude the meeting"
        );
    }

    #[test]
    fn missing_attendees_field_still_counts() {
        let events = vec![timed_event("2026-06-01T09:00:00Z", "2026-06-01T09:30:00Z")];
        let days = aggregate_by_day(&events);
        assert_eq!(days.len(), 1);
    }
}
