// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import CheckinForm from "../../src/components/forms/CheckinForm";

const mockUpsert = vi.fn();
vi.mock("../../src/api/checkins", () => ({
  checkinsApi: {
    upsert: (...args: unknown[]) => mockUpsert(...args),
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
    mockUpsert.mockReset();
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
    expect(screen.getByRole("button", { name: /save check-in/i })).toBeDefined();
  });

  it("submits correct data", async () => {
    mockUpsert.mockResolvedValue({
      id: "uuid-1",
      user_id: "user-1",
      date: "2026-03-18",
      energy: 7,
      mood: 8,
      focus: 6,
      recovery: 5,
      libido: 5,
      created_at: "2026-03-18T00:00:00Z",
    });

    renderWithProviders();
    const user = userEvent.setup();

    // Fill in the date
    const dateInput = screen.getByLabelText(/date/i);
    await user.clear(dateInput);
    await user.type(dateInput, "2026-03-18");

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
    await user.click(screen.getByRole("button", { name: /save check-in/i }));

    await waitFor(() => {
      expect(mockUpsert).toHaveBeenCalledOnce();
    });

    const submitted = mockUpsert.mock.calls[0][0];
    expect(submitted.date).toBe("2026-03-18");
    expect(submitted.energy).toBe(7);
    expect(submitted.mood).toBe(8);
    expect(submitted.focus).toBe(6);
    expect(submitted.recovery).toBe(5);
    expect(submitted.libido).toBe(5);
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
