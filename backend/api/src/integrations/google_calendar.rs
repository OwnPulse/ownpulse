// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Google Calendar API HTTP client and meeting-aggregation logic.
//!
//! **Privacy boundary**: this module deliberately never deserializes event
//! titles, descriptions, attendees, or locations — only `start`/`end` times.
//! `calendar_days` stores meeting *counts and minutes only* (see
//! `docs/decisions/0011-explore-and-observer-polls.md` and CLAUDE.md's
//! "Cooperative data boundary"); there is no code path in this module through
//! which meeting content could reach the database, logs, or an error
//! message.

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;

/// Build a short, sanitized error message for a non-2xx Google Calendar
/// response. The raw body is logged server-side only, at `debug` level —
/// never returned to the caller, since it could echo back request
/// parameters (see the same pattern in `integrations::oura`/`garmin`).
fn sanitized_upstream_error(context: &str, status: reqwest::StatusCode, body: &str) -> String {
    tracing::debug!(%status, body, context, "Google Calendar API returned a non-2xx response");
    format!("{context}: Google Calendar API returned HTTP {status}")
}

/// An event's start or end time — either a timed instant (`dateTime`) or an
/// all-day date string (`date`), per the Google Calendar API. Only these two
/// fields are ever deserialized from an event; see module docs.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventTime {
    pub date_time: Option<DateTime<Utc>>,
    #[allow(dead_code)]
    // kept for schema fidelity; distinguishes all-day events from malformed ones
    pub date: Option<String>,
}

/// A single calendar event's timing, with no other fields. Google's API
/// response also includes `summary`, `description`, `attendees`, and
/// `location` — none of those are deserialized here, so they never enter
/// this process's memory in a structured form.
#[derive(Debug, Deserialize)]
pub struct CalendarEvent {
    pub start: Option<EventTime>,
    pub end: Option<EventTime>,
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

    /// Fetch all events between `time_min` and `time_max` from the user's
    /// primary Google Calendar, handling pagination automatically.
    pub async fn fetch_events(
        &self,
        access_token: &str,
        time_min: DateTime<Utc>,
        time_max: DateTime<Utc>,
    ) -> Result<Vec<CalendarEvent>, String> {
        let mut all_events = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let url = format!("{}/calendar/v3/calendars/primary/events", self.api_base_url);
            let mut request = self.http.get(&url).bearer_auth(access_token).query(&[
                ("timeMin", time_min.to_rfc3339()),
                ("timeMax", time_max.to_rfc3339()),
                ("singleEvents", "true".to_string()),
                ("orderBy", "startTime".to_string()),
            ]);

            if let Some(ref token) = page_token {
                request = request.query(&[("pageToken", token.as_str())]);
            }

            let response = request
                .send()
                .await
                .map_err(|e| format!("calendar events request failed: {e}"))?;

            let status = response.status();
            if !status.is_success() {
                let body = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "unreadable body".into());
                return Err(sanitized_upstream_error("fetch events", status, &body));
            }

            let page: CalendarEventsResponse = response
                .json()
                .await
                .map_err(|e| format!("failed to parse calendar events response: {e}"))?;

            if let Some(items) = page.items {
                all_events.extend(items);
            }

            match page.next_page_token {
                Some(token) => page_token = Some(token),
                None => break,
            }
        }

        Ok(all_events)
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
/// blocks) carry only a `date` and aren't "meetings" in the sense this table
/// tracks, so they're skipped entirely. Zero- or negative-duration events
/// (malformed upstream data) are also skipped rather than recorded as a
/// meeting with nonsensical duration.
///
/// An event is bucketed by the UTC calendar date of its *start* time — a
/// meeting spanning midnight is not split across two days, it counts in
/// full on the day it started. This keeps aggregation a single pass with no
/// cross-day double counting.
pub fn aggregate_by_day(events: &[CalendarEvent]) -> BTreeMap<NaiveDate, DayAggregate> {
    let mut days: BTreeMap<NaiveDate, DayAggregate> = BTreeMap::new();

    for event in events {
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

    days
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timed_event(start: &str, end: &str) -> CalendarEvent {
        CalendarEvent {
            start: Some(EventTime {
                date_time: Some(start.parse().unwrap()),
                date: None,
            }),
            end: Some(EventTime {
                date_time: Some(end.parse().unwrap()),
                date: None,
            }),
        }
    }

    fn all_day_event(date: &str) -> CalendarEvent {
        CalendarEvent {
            start: Some(EventTime {
                date_time: None,
                date: Some(date.to_string()),
            }),
            end: Some(EventTime {
                date_time: None,
                date: Some(date.to_string()),
            }),
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
        let events = vec![all_day_event("2026-06-01")];
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
                    date: None,
                }),
            },
            CalendarEvent {
                start: Some(EventTime {
                    date_time: Some("2026-06-01T09:00:00Z".parse().unwrap()),
                    date: None,
                }),
                end: None,
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
}
