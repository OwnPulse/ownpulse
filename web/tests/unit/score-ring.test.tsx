// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ScoreRing } from "../../src/components/dashboard/ScoreRing";

describe("ScoreRing", () => {
  it("renders value and label", () => {
    render(<ScoreRing label="energy" value={7} />);
    expect(screen.getByText("7")).toBeDefined();
    expect(screen.getByText("energy")).toBeDefined();
  });

  it("renders dash when value is null", () => {
    render(<ScoreRing label="mood" value={null} />);
    expect(screen.getByText("\u2014")).toBeDefined();
    expect(screen.getByText("mood")).toBeDefined();
  });

  it("renders SVG with two circles for non-null value", () => {
    const { container } = render(<ScoreRing label="focus" value={5} />);
    const circles = container.querySelectorAll("circle");
    // Background track + progress arc
    expect(circles.length).toBe(2);
  });

  it("renders SVG with only background track for null value", () => {
    const { container } = render(<ScoreRing label="recovery" value={null} />);
    const circles = container.querySelectorAll("circle");
    // Only background track, no progress arc
    expect(circles.length).toBe(1);
  });

  it("sets correct stroke-dashoffset for progress", () => {
    const { container } = render(<ScoreRing label="energy" value={10} />);
    const progressCircle = container.querySelectorAll("circle")[1];
    // Full score (10/10) = offset should be 0
    const offset = Number(progressCircle.getAttribute("stroke-dashoffset"));
    expect(offset).toBeCloseTo(0, 1);
  });

  it("uses dimension-specific color", () => {
    const { container } = render(<ScoreRing label="energy" value={5} />);
    const bgCircle = container.querySelector("circle");
    // Energy = gold #c49a3c
    expect(bgCircle?.getAttribute("stroke")).toBe("#c49a3c");
  });

  it("uses fallback color for unknown label", () => {
    const { container } = render(<ScoreRing label="unknown" value={5} />);
    const bgCircle = container.querySelector("circle");
    expect(bgCircle?.getAttribute("stroke")).toBe("#999");
  });
});
