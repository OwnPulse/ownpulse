// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DIMENSION_COLORS } from "../../src/components/dimensionColors.generated";
import CheckinForm from "../../src/components/forms/CheckinForm";

const mockCreate = vi.fn();
vi.mock("../../src/api/checkins", () => ({
  checkinsApi: {
    create: (...args: unknown[]) => mockCreate(...args),
  },
}));

function renderWithProviders() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <CheckinForm />
    </QueryClientProvider>,
  );
}

describe("CheckinForm", () => {
  beforeEach(() => {
    mockCreate.mockReset();
  });

  it("renders score inputs", () => {
    renderWithProviders();

    expect(screen.getByLabelText(/date/i)).toBeDefined();
    expect(screen.getByLabelText(/energy/i)).toBeDefined();
    expect(screen.getByLabelText(/mood/i)).toBeDefined();
    expect(screen.getByLabelText(/focus/i)).toBeDefined();
    expect(screen.getByLabelText(/recovery/i)).toBeDefined();
    expect(screen.getByLabelText(/libido/i)).toBeDefined();
    expect(screen.getByLabelText(/notes/i)).toBeDefined();
    expect(screen.getByRole("button", { name: /log check-in/i })).toBeDefined();
  });

  it("exposes slider values as accessible valuetext", () => {
    renderWithProviders();

    const energySlider = screen.getByRole("slider", { name: /energy/i });
    expect(energySlider.getAttribute("aria-valuetext")).toBe("5 out of 10");

    fireInputChange(energySlider, "8");
    expect(energySlider.getAttribute("aria-valuetext")).toBe("8 out of 10");
  });

  it("mounts an accessible status region before submit, and updates its text (rather than mounting a new node) on success", async () => {
    mockCreate.mockResolvedValue({
      id: "uuid-1",
      user_id: "user-1",
      date: "2026-03-18", // date-ok
      energy: 5,
      mood: 5,
      focus: 5,
      recovery: 5,
      libido: 5,
      created_at: "2026-03-18T00:00:00Z", // date-ok
    });

    renderWithProviders();
    const user = userEvent.setup();

    // The status container must already exist (and be empty) before submit —
    // a role="status" node that only mounts once the mutation settles isn't
    // reliably announced by screen readers.
    const status = screen.getByRole("status");
    expect(status.textContent).toBe("");

    await user.click(screen.getByRole("button", { name: /log check-in/i }));

    await waitFor(() => {
      expect(status.textContent).toMatch(/saved/i);
    });
    // Same node throughout — confirms the container wasn't unmounted/remounted.
    expect(screen.getByRole("status")).toBe(status);
  });

  it("submits correct data", async () => {
    mockCreate.mockResolvedValue({
      id: "uuid-1",
      user_id: "user-1",
      date: "2026-03-18", // date-ok
      energy: 7,
      mood: 8,
      focus: 6,
      recovery: 5,
      libido: 5,
      created_at: "2026-03-18T00:00:00Z", // date-ok
    });

    renderWithProviders();
    const user = userEvent.setup();

    // Fill in the date
    const dateInput = screen.getByLabelText(/date/i);
    await user.clear(dateInput);
    await user.type(dateInput, "2026-03-18"); // date-ok

    // Change energy slider to 7
    const energySlider = screen.getByLabelText(/energy/i);
    fireInputChange(energySlider, "7");

    // Change mood slider to 8
    const moodSlider = screen.getByLabelText(/mood/i);
    fireInputChange(moodSlider, "8");

    // Change focus slider to 6
    const focusSlider = screen.getByLabelText(/focus/i);
    fireInputChange(focusSlider, "6");

    // Submit
    await user.click(screen.getByRole("button", { name: /log check-in/i }));

    await waitFor(() => {
      expect(mockCreate).toHaveBeenCalledOnce();
    });

    const submitted = mockCreate.mock.calls[0][0];
    expect(submitted.date).toBe("2026-03-18"); // date-ok
    expect(submitted.energy).toBe(7);
    expect(submitted.mood).toBe(8);
    expect(submitted.focus).toBe(6);
    expect(submitted.recovery).toBe(5);
    expect(submitted.libido).toBe(5);
  });

  it("tints each slider with its DIMENSION_COLORS entry", () => {
    // CheckinForm looks up DIMENSION_COLORS[label.toLowerCase()]; a label that
    // doesn't lowercase-match a DIMENSION_COLORS key yields `undefined` and
    // silently drops the tint, so this asserts the actual resolved color per
    // slider rather than just that *some* background is set.
    renderWithProviders();
    for (const [label, key] of [
      ["energy", "energy"],
      ["mood", "mood"],
      ["focus", "focus"],
      ["recovery", "recovery"],
      ["libido", "libido"],
    ] as const) {
      const slider = screen.getByLabelText(new RegExp(label, "i"));
      expect(slider.style.background).toContain(DIMENSION_COLORS[key]);
    }
  });

  describe("default date at a UTC-offset-sensitive instant", () => {
    const originalTz = process.env.TZ;

    beforeEach(() => {
      process.env.TZ = "Pacific/Honolulu"; // UTC-10, no DST
    });

    afterEach(() => {
      vi.useRealTimers();
      if (originalTz === undefined) {
        delete process.env.TZ;
      } else {
        process.env.TZ = originalTz;
      }
    });

    it("defaults the date field from src/utils/datetime's localToday(), not a UTC-derived value", () => {
      // 2026-03-01T05:30:00Z is 2026-02-28T19:30 in Honolulu. A form that
      // reverted to `new Date().toISOString().slice(0, 10)` would default to
      // "2026-03-01" here — this test would still pass if that regression
      // landed unless it pins the local value explicitly.
      vi.setSystemTime(new Date("2026-03-01T05:30:00Z")); // date-ok

      renderWithProviders();

      const dateInput = screen.getByLabelText(/date/i) as HTMLInputElement;
      expect(dateInput.value).toBe("2026-02-28"); // date-ok
    });
  });
});

/** Helper to change an input value (range inputs don't respond to userEvent.type). */
function fireInputChange(el: HTMLElement, value: string) {
  const nativeInputValueSetter = Object.getOwnPropertyDescriptor(
    HTMLInputElement.prototype,
    "value",
  )?.set;
  if (nativeInputValueSetter) {
    nativeInputValueSetter.call(el, value);
  }
  el.dispatchEvent(new Event("change", { bubbles: true }));
}
