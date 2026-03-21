// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import SleepChart from "../../src/components/SleepChart";
import type { SleepRecord } from "../../src/api/sleep";

// Unovis uses SVG and D3 under the hood which don't render meaningfully in jsdom.
// Mock the unovis components so we can test SleepChart's own logic in isolation.
vi.mock("@unovis/react", () => ({
  VisXYContainer: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="xy-container">{children}</div>
  ),
  VisStackedBar: () => <div data-testid="stacked-bar" />,
  VisAxis: ({ type, label }: { type?: string; label?: string }) => (
    <div data-testid={`axis-${type ?? label}`} />
  ),
}));

function makeSleepRecord(overrides: Partial<SleepRecord> = {}): SleepRecord {
  return {
    id: "uuid-1",
    user_id: "user-1",
    date: "2026-03-10",
    sleep_start: "2026-03-09T23:00:00Z",
    sleep_end: "2026-03-10T07:00:00Z",
    duration_minutes: 480,
    deep_minutes: 90,
    light_minutes: 210,
    rem_minutes: 120,
    awake_minutes: 60,
    score: 82,
    source: "manual",
    source_id: null,
    notes: null,
    created_at: "2026-03-10T08:00:00Z",
    ...overrides,
  };
}

describe("SleepChart", () => {
  it("renders the chart container and bars when data is provided", () => {
    const data = [
      makeSleepRecord({ id: "uuid-1", date: "2026-03-10" }),
      makeSleepRecord({ id: "uuid-2", date: "2026-03-11" }),
    ];

    render(<SleepChart data={data} />);

    expect(screen.getByTestId("xy-container")).toBeDefined();
    expect(screen.getByTestId("stacked-bar")).toBeDefined();
  });

  it("renders x and y axes when data is provided", () => {
    const data = [makeSleepRecord()];

    render(<SleepChart data={data} />);

    expect(screen.getByTestId("axis-x")).toBeDefined();
    expect(screen.getByTestId("axis-y")).toBeDefined();
  });

  it("shows empty state message when data array is empty", () => {
    render(<SleepChart data={[]} />);

    expect(screen.getByText(/no sleep data/i)).toBeDefined();
    expect(screen.queryByTestId("xy-container")).toBeNull();
  });

  it("renders without errors when all stage minutes are null", () => {
    const data = [
      makeSleepRecord({
        deep_minutes: null,
        light_minutes: null,
        rem_minutes: null,
        awake_minutes: null,
      }),
    ];

    render(<SleepChart data={data} />);

    expect(screen.getByTestId("xy-container")).toBeDefined();
    expect(screen.getByTestId("stacked-bar")).toBeDefined();
  });

  it("does not log any health data to the console", () => {
    const consoleSpy = vi.spyOn(console, "log");
    const data = [makeSleepRecord()];

    render(<SleepChart data={data} />);

    expect(consoleSpy).not.toHaveBeenCalled();
    consoleSpy.mockRestore();
  });
});
