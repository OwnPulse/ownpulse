// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import DataEntry from "../../src/pages/DataEntry";

// Mock all form components to avoid pulling in their dependencies
vi.mock("../../src/components/forms/CheckinForm", () => ({
  default: () => <div data-testid="checkin-form">CheckinForm</div>,
}));
vi.mock("../../src/components/forms/InterventionForm", () => ({
  default: () => <div data-testid="intervention-form">InterventionForm</div>,
}));
vi.mock("../../src/components/forms/HealthRecordForm", () => ({
  default: () => <div data-testid="health-record-form">HealthRecordForm</div>,
}));
vi.mock("../../src/components/forms/ObservationForm", () => ({
  default: () => <div data-testid="observation-form">ObservationForm</div>,
}));
vi.mock("../../src/components/forms/LabResultForm", () => ({
  default: () => <div data-testid="lab-result-form">LabResultForm</div>,
}));

function renderWithProviders() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <DataEntry />
    </QueryClientProvider>,
  );
}

describe("DataEntry", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders all 5 tabs", () => {
    renderWithProviders();

    expect(screen.getByText("Check-in")).toBeDefined();
    expect(screen.getByText("Intervention")).toBeDefined();
    expect(screen.getByText("Health Record")).toBeDefined();
    expect(screen.getByText("Observation")).toBeDefined();
    expect(screen.getByText("Lab Result")).toBeDefined();
  });

  it("switching tabs toggles the hidden attribute on each tabpanel (panels stay mounted so aria-controls always points at a real element)", async () => {
    renderWithProviders();
    const user = userEvent.setup();

    const checkinPanel = screen.getByTestId("checkin-form").parentElement as HTMLElement;
    const interventionPanel = screen.getByTestId("intervention-form").parentElement as HTMLElement;
    const labResultPanel = screen.getByTestId("lab-result-form").parentElement as HTMLElement;

    // Default tab is Check-in — all panels are mounted, only the active one is unhidden
    expect(checkinPanel).not.toHaveAttribute("hidden");
    expect(interventionPanel).toHaveAttribute("hidden");
    expect(labResultPanel).toHaveAttribute("hidden");

    // Switch to Intervention
    await user.click(screen.getByText("Intervention"));
    expect(interventionPanel).not.toHaveAttribute("hidden");
    expect(checkinPanel).toHaveAttribute("hidden");

    // Switch to Lab Result
    await user.click(screen.getByText("Lab Result"));
    expect(labResultPanel).not.toHaveAttribute("hidden");
    expect(interventionPanel).toHaveAttribute("hidden");
  });

  it("exposes an accessible tablist with the active tab selected", () => {
    renderWithProviders();

    const tablist = screen.getByRole("tablist", { name: /data entry type/i });
    expect(tablist).toBeDefined();

    const tabs = screen.getAllByRole("tab");
    expect(tabs).toHaveLength(5);

    const checkinTab = screen.getByRole("tab", { name: "Check-in" });
    expect(checkinTab.getAttribute("aria-selected")).toBe("true");

    const interventionTab = screen.getByRole("tab", { name: "Intervention" });
    expect(interventionTab.getAttribute("aria-selected")).toBe("false");
  });

  it("switching tabs via click updates aria-selected and the tabpanel", async () => {
    renderWithProviders();
    const user = userEvent.setup();

    await user.click(screen.getByRole("tab", { name: "Intervention" }));

    expect(screen.getByRole("tab", { name: "Intervention" }).getAttribute("aria-selected")).toBe(
      "true",
    );
    expect(screen.getByRole("tab", { name: "Check-in" }).getAttribute("aria-selected")).toBe(
      "false",
    );
    expect(screen.getByRole("tabpanel")).toBeDefined();
  });

  it("arrow keys move focus and selection between tabs", async () => {
    renderWithProviders();
    const user = userEvent.setup();

    const checkinTab = screen.getByRole("tab", { name: "Check-in" });
    checkinTab.focus();

    await user.keyboard("{ArrowRight}");

    const interventionTab = screen.getByRole("tab", { name: "Intervention" });
    expect(interventionTab.getAttribute("aria-selected")).toBe("true");
    expect(interventionTab).toHaveFocus();
  });

  it("Home/End keys jump to the first/last tab", async () => {
    renderWithProviders();
    const user = userEvent.setup();

    const checkinTab = screen.getByRole("tab", { name: "Check-in" });
    checkinTab.focus();

    await user.keyboard("{End}");
    const labResultTab = screen.getByRole("tab", { name: "Lab Result" });
    expect(labResultTab.getAttribute("aria-selected")).toBe("true");
    expect(labResultTab).toHaveFocus();

    await user.keyboard("{Home}");
    expect(checkinTab.getAttribute("aria-selected")).toBe("true");
    expect(checkinTab).toHaveFocus();
  });

  it("every tab's aria-controls resolves to a real (if hidden) tabpanel element", () => {
    renderWithProviders();

    for (const tab of screen.getAllByRole("tab")) {
      const controlsId = tab.getAttribute("aria-controls");
      expect(controlsId).toBeTruthy();
      expect(document.getElementById(controlsId as string)).not.toBeNull();
    }
  });
});
