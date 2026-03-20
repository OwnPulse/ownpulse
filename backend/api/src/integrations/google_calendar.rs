// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Google Calendar API HTTP client for fetching calendar events.

use chrono::{DateTime, Utc};
use serde::Deserialize;

/// A single calendar event from the Google Calendar API.
#[derive(Debug, Deserialize)]
pub struct CalendarEvent {
    pub summary: Option<String>,
    pub start: Option<EventTime>,
    pub end: Option<EventTime>,
}

/// An event time, which can be either a date-time or an all-day date string.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventTime {
    pub date_time: Option<DateTime<Utc>>,
    pub date: Option<String>,
}

/// Response envelope from the Google Calendar events list endpoint.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEventsResponse {
    pub items: Option<Vec<CalendarEvent>>,
    pub next_page_token: Option<String>,
}

/// Fetch all events between `time_min` and `time_max` from the user's primary
/// Google Calendar, handling pagination automatically.
pub async fn fetch_events(
    client: &reqwest::Client,
    access_token: &str,
    time_min: DateTime<Utc>,
    time_max: DateTime<Utc>,
) -> Result<Vec<CalendarEvent>, String> {
    let mut all_events = Vec::new();
    let mut page_token: Option<String> = None;

    loop {
        let mut request = client
            .get("https://www.googleapis.com/calendar/v3/calendars/primary/events")
            .bearer_auth(access_token)
            .query(&[
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

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unreadable body".into());
            return Err(format!(
                "calendar events returned {status}: {body}"
            ));
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
