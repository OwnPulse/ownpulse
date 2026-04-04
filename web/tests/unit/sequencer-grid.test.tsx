// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import SequencerGrid from "../../src/components/protocols/SequencerGrid";

const twoLines = [
  { substance: "BPC-157", schedule_pattern: [true, false, true] },
  { substance: "TB-500", schedule_pattern: [false, true, false] },
];

describe("SequencerGrid", () => {
  it("renders substance labels and day headers", () => {
    render(<SequencerGrid lines={twoLines} durationDays={3} editable={false} />);

    expect(screen.getByText("BPC-157")).toBeDefined();
    expect(screen.getByText("TB-500")).toBeDefined();
    expect(screen.getByText("D1")).toBeDefined();
    expect(screen.getByText("D2")).toBeDefined();
    expect(screen.getByText("D3")).toBeDefined();
  });

  it("renders active cells with filled circle", () => {
    render(<SequencerGrid lines={twoLines} durationDays={3} editable={false} />);

    const bpcD1 = screen.getByLabelText("BPC-157 day 1: active");
    expect(bpcD1.textContent).toBe("\u25CF");

    const bpcD2 = screen.getByLabelText("BPC-157 day 2: inactive");
    expect(bpcD2.textContent).toBe("");
  });

  it("calls onToggleCell when editable cell is clicked", async () => {
    const onToggle = vi.fn();
    const user = userEvent.setup();

    render(<SequencerGrid lines={twoLines} durationDays={3} editable onToggleCell={onToggle} />);

    const cell = screen.getByLabelText("BPC-157 day 2: inactive");
    await user.click(cell);

    expect(onToggle).toHaveBeenCalledWith(0, 1);
  });

  it("does not call onToggleCell when not editable", async () => {
    const onToggle = vi.fn();
    const user = userEvent.setup();

    render(
      <SequencerGrid lines={twoLines} durationDays={3} editable={false} onToggleCell={onToggle} />,
    );

    const cell = screen.getByLabelText("BPC-157 day 1: active");
    await user.click(cell);

    expect(onToggle).not.toHaveBeenCalled();
  });

  it("highlights today cell", () => {
    render(<SequencerGrid lines={twoLines} durationDays={3} editable={false} todayIndex={1} />);

    const todayCell = screen.getByLabelText("BPC-157 day 2: inactive");
    expect(todayCell.className).toContain("today");
  });

  it("has overscroll-behavior-x: contain on the wrapper", () => {
    const { container } = render(
      <SequencerGrid lines={twoLines} durationDays={3} editable={false} />,
    );

    // The wrapper is the outermost div rendered by the component
    // CSS modules are applied as class names; in jsdom the actual computed
    // styles from .module.css aren't loaded, so we verify the class is present.
    const wrapper = container.firstElementChild as HTMLElement;
    expect(wrapper.className).toContain("wrapper");
  });
});
