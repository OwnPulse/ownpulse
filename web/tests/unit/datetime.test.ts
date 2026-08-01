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
    // Assigning `undefined` coerces to the string "undefined" (a silent UTC
    // fallback for every later test in the run) rather than unsetting TZ.
    if (originalTz === undefined) {
      delete process.env.TZ;
    } else {
      process.env.TZ = originalTz;
    }
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

  it("localNow reads the clock exactly once, so its date and time halves can never disagree", () => {
    // If localNow() constructed a second Date internally (e.g. by delegating
    // its date portion to localToday()), a real midnight straddle between
    // the two reads could return yesterday's date with today's time. Asserting
    // exactly one construction is a direct regression test for that, since a
    // frozen fake-timer instant can't otherwise expose a race between two reads.
    const RealDate = globalThis.Date;
    let constructCount = 0;
    class CountingDate extends RealDate {
      constructor(...args: ConstructorParameters<typeof Date>) {
        super(...args);
        constructCount++;
      }
    }
    vi.stubGlobal("Date", CountingDate);

    localNow();

    expect(constructCount).toBe(1);
    vi.unstubAllGlobals();
  });
});
