// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { MemoryRouter } from "react-router-dom";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { TemplateListItem } from "../../src/api/protocols";
import { ImportModal } from "../../src/components/protocols/ImportModal";
import PatternSelector from "../../src/components/protocols/PatternSelector";
import SequencerGrid from "../../src/components/protocols/SequencerGrid";
import { TemplateCard } from "../../src/components/protocols/TemplateCard";
import ProtocolBuilder from "../../src/pages/ProtocolBuilder";
import { useAuthStore } from "../../src/store/auth";

const interventionsList = [
  {
    id: "iv-1",
    user_id: "user-1",
    substance: "Caffeine",
    dose: 200,
    unit: "mg",
    route: "oral",
    administered_at: "2026-03-02T08:00:00Z",
    fasted: false,
    created_at: "2026-03-02T08:00:00Z",
  },
  {
    id: "iv-2",
    user_id: "user-1",
    substance: "Vitamin D3",
    dose: 5000,
    unit: "IU",
    route: "oral",
    administered_at: "2026-03-03T08:00:00Z",
    fasted: false,
    created_at: "2026-03-03T08:00:00Z",
  },
];

const server = setupServer(
  http.get("/api/v1/interventions", () => {
    return HttpResponse.json(interventionsList);
  }),
);

beforeAll(() => server.listen());
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

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

  it("renders weekday labels when mode is weekday", () => {
    // 2026-03-28 is a Saturday
    const { container } = render(
      <SequencerGrid
        lines={lines}
        durationDays={7}
        editable={false}
        dayLabelMode="weekday"
        startDate="2026-03-28"
      />,
    );

    const dayLabels = container.querySelectorAll("[class*='dayNumber']");
    const labels = Array.from(dayLabels).map((el) => el.textContent);
    expect(labels).toEqual(["Sat", "Sun", "Mon", "Tue", "Wed", "Thu", "Fri"]);
  });

  it("renders numbered labels by default", () => {
    const { container } = render(<SequencerGrid lines={lines} durationDays={7} editable={false} />);

    const dayLabels = container.querySelectorAll("[class*='dayNumber']");
    const labels = Array.from(dayLabels).map((el) => el.textContent);
    expect(labels).toEqual(["D1", "D2", "D3", "D4", "D5", "D6", "D7"]);
  });

  it("renders copy-week-forward button when showCopyWeek is true", () => {
    const fourteenDayLines = [
      {
        substance: "Test",
        schedule_pattern: Array(14).fill(true),
      },
    ];
    const onCopy = vi.fn();
    render(
      <SequencerGrid
        lines={fourteenDayLines}
        durationDays={14}
        editable={true}
        showCopyWeek={true}
        onCopyWeekForward={onCopy}
      />,
    );

    // Week 1 should have a copy button since there are more weeks after it
    const copyBtn = screen.getByLabelText("Copy week 1 forward");
    expect(copyBtn).toBeDefined();
  });

  it("calls onCopyWeekForward with correct week index", async () => {
    const fourteenDayLines = [
      {
        substance: "Test",
        schedule_pattern: Array(14).fill(true),
      },
    ];
    const onCopy = vi.fn();
    render(
      <SequencerGrid
        lines={fourteenDayLines}
        durationDays={14}
        editable={true}
        showCopyWeek={true}
        onCopyWeekForward={onCopy}
      />,
    );

    const copyBtn = screen.getByLabelText("Copy week 1 forward");
    await userEvent.click(copyBtn);

    expect(onCopy).toHaveBeenCalledWith(0);
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

  it("generates 3x per Week pattern", async () => {
    const onSelect = vi.fn();
    render(<PatternSelector durationDays={7} onSelect={onSelect} />);

    const select = screen.getByLabelText("Schedule pattern");
    await userEvent.selectOptions(select, "3x per Week");

    // D1, D3, D5 = [T, F, T, F, T, F, F]
    expect(onSelect).toHaveBeenCalledWith([true, false, true, false, true, false, false]);
  });

  it("generates Twice a Week pattern", async () => {
    const onSelect = vi.fn();
    render(<PatternSelector durationDays={7} onSelect={onSelect} />);

    const select = screen.getByLabelText("Schedule pattern");
    await userEvent.selectOptions(select, "Twice a Week");

    // D1, D4 = [T, F, F, T, F, F, F]
    expect(onSelect).toHaveBeenCalledWith([true, false, false, true, false, false, false]);
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

describe("ProtocolBuilder", () => {
  beforeEach(() => {
    useAuthStore.getState().login("test-jwt-token");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  it("renders duration preset buttons", () => {
    withProviders(<ProtocolBuilder />);

    expect(screen.getByRole("button", { name: "2W" })).toBeDefined();
    expect(screen.getByRole("button", { name: "4W" })).toBeDefined();
    expect(screen.getByRole("button", { name: "8W" })).toBeDefined();
    expect(screen.getByRole("button", { name: "12W" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Custom" })).toBeDefined();
  });

  it("clicking preset changes duration", async () => {
    withProviders(<ProtocolBuilder />);

    // Default is 4W, click 8W
    const btn8w = screen.getByRole("button", { name: "8W" });
    await userEvent.click(btn8w);

    // The 8W button should now be active (has durationActive class)
    expect(btn8w.className).toContain("durationActive");
  });

  it("clicking Custom shows days input", async () => {
    withProviders(<ProtocolBuilder />);

    const customBtn = screen.getByRole("button", { name: "Custom" });
    await userEvent.click(customBtn);

    const input = screen.getByLabelText("Custom duration in days");
    expect(input).toBeDefined();
    expect((input as HTMLInputElement).value).toBe("28");
  });

  it("uses stable key for line cards (index-based)", async () => {
    withProviders(<ProtocolBuilder />);

    // Type in the substance field
    const substanceInput = screen.getByLabelText("Substance");
    await userEvent.type(substanceInput, "BPC");

    // The input should still be focused and have the typed value
    expect(document.activeElement).toBe(substanceInput);
    expect((substanceInput as HTMLInputElement).value).toBe("BPC");
  });

  it("renders substance datalist with suggestions", async () => {
    withProviders(<ProtocolBuilder />);

    // Wait for interventions to load
    await waitFor(() => {
      const datalist = document.getElementById("substance-suggestions-0");
      expect(datalist).not.toBeNull();
      const options = datalist?.querySelectorAll("option");
      // Should have at least the common substances + user substances
      expect(options?.length).toBeGreaterThan(0);
    });
  });

  it("merges user intervention substances with common list", async () => {
    withProviders(<ProtocolBuilder />);

    await waitFor(() => {
      const datalist = document.getElementById("substance-suggestions-0");
      const options = datalist?.querySelectorAll("option");
      const values = Array.from(options ?? []).map((o) => o.getAttribute("value"));
      // "Caffeine" comes from user interventions, "BPC-157" from common list
      expect(values).toContain("Caffeine");
      expect(values).toContain("BPC-157");
      // Vitamin D3 appears in both user interventions and common list, should be deduplicated
      const vitD3Count = values.filter((v) => v === "Vitamin D3").length;
      expect(vitD3Count).toBe(1);
    });
  });

  it("renders pattern selector label", () => {
    withProviders(<ProtocolBuilder />);

    expect(screen.getByText("Schedule:")).toBeDefined();
  });

  it("shows duration label with weeks for clean multiples", () => {
    withProviders(<ProtocolBuilder />);

    // Default is 28 days = 4 weeks; the label format includes "Duration —"
    expect(screen.getByText(/Duration — 28 days \(4 weeks\)/)).toBeDefined();
  });

  it("renders template mode toggle", () => {
    withProviders(<ProtocolBuilder />);

    expect(screen.getByRole("button", { name: "Week Template" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Full Schedule" })).toBeDefined();
  });

  it("defaults to template mode with hint text", () => {
    withProviders(<ProtocolBuilder />);

    expect(screen.getByText(/Edit one week below/)).toBeDefined();
  });

  it("switching to Full Schedule hides template hint and shows day label toggle", async () => {
    withProviders(<ProtocolBuilder />);

    const fullBtn = screen.getByRole("button", { name: "Full Schedule" });
    await userEvent.click(fullBtn);

    expect(screen.queryByText(/Edit one week below/)).toBeNull();
    expect(screen.getByLabelText("Show numbered days")).toBeDefined();
    expect(screen.getByLabelText("Show weekday names")).toBeDefined();
  });

  it("Full Schedule shows Add Week button", async () => {
    withProviders(<ProtocolBuilder />);

    await userEvent.click(screen.getByRole("button", { name: "Full Schedule" }));

    expect(screen.getByLabelText("Add one week")).toBeDefined();
  });
});
