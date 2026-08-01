// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { localNow, localToday } from "../../src/utils/datetime";

// Set a fixed, negative-offset timezone (UTC-10) BEFORE any Date is constructed
// in this file, so that `new Date(...)`'s local getters (getFullYear,
// getHours, etc.) — which is exactly what src/utils/datetime.ts relies on —
// resolve against Honolulu local time rather than the CI runner's timezone.
// This is set directly on process.env.TZ (not via vi.stubEnv, which only
// patches vitest's view of process.env and does not reliably reach the
// native Date/Intl timezone lookup) so it deterministically shifts Date's
// local-time getters.
const originalTz = process.env.TZ;

describe("datetime", () => {
  beforeEach(() => {
    process.env.TZ = "Pacific/Honolulu"; // UTC-10, no DST
  });

  afterEach(() => {
    vi.useRealTimers();
    process.env.TZ = originalTz;
  });

  it("localToday returns yesterday's local date when it is just after UTC midnight", async () => {
    // 2026-03-01T05:30:00Z is 2026-02-28T19:30 in Honolulu (UTC-10).
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-03-01T05:30:00Z"));

    expect(localToday()).toBe("2026-02-28");
    // The UTC-based approach this replaces would have produced tomorrow's
    // date relative to the user's local day.
    expect(new Date().toISOString().slice(0, 10)).toBe("2026-03-01");
  });

  it("localToday returns the correct local date well after local midnight", async () => {
    // 2026-03-01T20:00:00Z is 2026-03-01T10:00 in Honolulu — same calendar day.
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-03-01T20:00:00Z"));

    expect(localToday()).toBe("2026-03-01");
  });

  it("localToday zero-pads single-digit months and days", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-01-15T20:00:00Z")); // 2026-01-15T10:00 local

    expect(localToday()).toBe("2026-01-15");
  });

  it("localNow returns yesterday's local date and time near UTC midnight", async () => {
    // 2026-03-01T05:09:00Z is 2026-02-28T19:09 in Honolulu.
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-03-01T05:09:00Z"));

    expect(localNow()).toBe("2026-02-28T19:09");
  });

  it("localNow zero-pads single-digit hours and minutes", async () => {
    // 2026-03-01T15:05:00Z is 2026-03-01T05:05 in Honolulu.
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-03-01T15:05:00Z"));

    expect(localNow()).toBe("2026-03-01T05:05");
  });

  it("localNow's date portion matches localToday for the same instant", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-15T12:34:00Z"));

    expect(localNow().slice(0, 10)).toBe(localToday());
  });
});
