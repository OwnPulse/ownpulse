// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import type { TemplateListItem } from "../../src/api/protocols";
import { ImportModal } from "../../src/components/protocols/ImportModal";
import PatternSelector from "../../src/components/protocols/PatternSelector";
import SequencerGrid from "../../src/components/protocols/SequencerGrid";
import { TemplateCard } from "../../src/components/protocols/TemplateCard";

function withProviders(ui: React.ReactElement) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>{ui}</MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("SequencerGrid", () => {
  const lines = [
    { substance: "Vitamin D", schedule_pattern: [true, false, true, false, true, false, true] },
    { substance: "Magnesium", schedule_pattern: [true, true, true, true, true, true, true] },
  ];

  it("renders correct number of cells", () => {
    render(<SequencerGrid lines={lines} durationDays={7} editable={false} />);

    // Each line gets 7 day cells => 14 total
    const cells = screen.getAllByRole("button");
    expect(cells).toHaveLength(14);
  });

  it("calls onToggleCell when cell clicked", async () => {
    const onToggle = vi.fn();
    render(
      <SequencerGrid lines={lines} durationDays={7} editable={true} onToggleCell={onToggle} />,
    );

    // Click the first cell of the second line (Magnesium, day 1)
    const cell = screen.getByLabelText("Magnesium day 1: active");
    await userEvent.click(cell);

    expect(onToggle).toHaveBeenCalledWith(1, 0);
  });

  it("highlights today column", () => {
    render(<SequencerGrid lines={lines} durationDays={7} editable={false} todayIndex={3} />);

    // Day 4 (index 3) for Vitamin D should have "today" in its class
    const todayCell = screen.getByLabelText("Vitamin D day 4: inactive");
    expect(todayCell.className).toContain("today");

    // Day 1 should not have "today"
    const otherCell = screen.getByLabelText("Vitamin D day 1: active");
    expect(otherCell.className).not.toContain("today");
  });
});

describe("PatternSelector", () => {
  it("generates daily pattern", async () => {
    const onSelect = vi.fn();
    render(<PatternSelector durationDays={7} onSelect={onSelect} />);

    const select = screen.getByLabelText("Schedule pattern");
    await userEvent.selectOptions(select, "Daily");

    expect(onSelect).toHaveBeenCalledWith([true, true, true, true, true, true, true]);
  });

  it("generates MWF pattern", async () => {
    const onSelect = vi.fn();
    render(<PatternSelector durationDays={7} onSelect={onSelect} />);

    const select = screen.getByLabelText("Schedule pattern");
    await userEvent.selectOptions(select, "MWF");

    // Mon, Wed, Fri = [T, F, T, F, T, F, F]
    expect(onSelect).toHaveBeenCalledWith([true, false, true, false, true, false, false]);
  });
});

describe("ImportModal", () => {
  it("parses valid JSON file", async () => {
    const validProtocol = JSON.stringify({
      schema: "ownpulse/protocol/v1",
      name: "Test Protocol",
      duration_days: 14,
      tags: [],
      lines: [{ substance: "Vitamin C", pattern: "daily" }],
    });

    const file = new File([validProtocol], "protocol.json", { type: "application/json" });

    withProviders(<ImportModal onClose={vi.fn()} />);

    const input = document.querySelector('input[type="file"]') as HTMLInputElement;
    await userEvent.upload(input, file);

    await waitFor(() => {
      expect(screen.getByText("Test Protocol")).toBeDefined();
    });
  });

  it("rejects invalid JSON", async () => {
    const file = new File(["not valid json{{{"], "bad.json", { type: "application/json" });

    withProviders(<ImportModal onClose={vi.fn()} />);

    const input = document.querySelector('input[type="file"]') as HTMLInputElement;
    await userEvent.upload(input, file);

    await waitFor(() => {
      expect(screen.getByText("Invalid JSON file.")).toBeDefined();
    });
  });
});

describe("TemplateCard", () => {
  const mockTemplate: TemplateListItem = {
    id: "tpl-1",
    name: "Nootropic Stack",
    description: "A basic cognitive enhancement stack",
    tags: ["nootropic", "cognitive"],
    duration_days: 30,
    line_count: 3,
  };

  it("renders template info", () => {
    withProviders(<TemplateCard template={mockTemplate} />);

    expect(screen.getByText("Nootropic Stack")).toBeDefined();
    expect(screen.getByText("A basic cognitive enhancement stack")).toBeDefined();
    expect(screen.getByText("nootropic")).toBeDefined();
    expect(screen.getByText("cognitive")).toBeDefined();
    expect(screen.getByText(/30 days/)).toBeDefined();
    expect(screen.getByText(/3 substances/)).toBeDefined();
  });

  it("copy flow prompts for start date", async () => {
    withProviders(<TemplateCard template={mockTemplate} />);

    const useBtn = screen.getByRole("button", { name: /use this protocol/i });
    await userEvent.click(useBtn);

    // Date picker should appear
    const dateInput = screen.getByDisplayValue(new Date().toISOString().slice(0, 10));
    expect(dateInput).toBeDefined();
    expect(screen.getByRole("button", { name: /start/i })).toBeDefined();
  });
});
