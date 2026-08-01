// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

/**
 * Local-time date/time helpers for entry form defaults.
 *
 * `new Date().toISOString()` always renders in UTC, so building a "today"
 * or "now" default from it shifts the date (and time) for any user west of
 * UTC — most visibly near midnight, where it can show tomorrow's date.
 * These helpers build the string from the local date/time parts instead.
 */

function pad(n: number): string {
  return n.toString().padStart(2, "0");
}

function formatLocalDate(d: Date): string {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** Local calendar date as YYYY-MM-DD, suitable for `<input type="date">`. */
export function localToday(): string {
  return formatLocalDate(new Date());
}

/** Local date + time as YYYY-MM-DDTHH:mm, suitable for `<input type="datetime-local">`. */
export function localNow(): string {
  // Read the clock once — calling localToday() here would construct a second
  // Date, and at a midnight straddle the date and time halves could come
  // from different instants (e.g. yesterday's date with today's time).
  const d = new Date();
  return `${formatLocalDate(d)}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}
