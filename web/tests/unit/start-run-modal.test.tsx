// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { StartRunModal } from "../../src/components/protocols/StartRunModal";
import { useAuthStore } from "../../src/store/auth";

const server = setupServer();

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

function renderModal(props?: { onClose?: () => void; onStarted?: () => void }) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  const onClose = props?.onClose ?? vi.fn();
  const onStarted = props?.onStarted ?? vi.fn();

  const result = render(
    <QueryClientProvider client={queryClient}>
      <StartRunModal
        protocolId="proto-1"
        protocolName="BPC-157 Stack"
        onClose={onClose}
        onStarted={onStarted}
      />
    </QueryClientProvider>,
  );

  return { ...result, onClose, onStarted };
}

describe("StartRunModal", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: "test-jwt", isAuthenticated: true });
  });

  it("renders with protocol name, date input, and submit button", () => {
    server.use(http.post("/api/v1/protocols/:id/runs", () => new Promise(() => {})));

    renderModal();

    expect(screen.getByRole("heading", { name: "Start Run" })).toBeDefined();
    expect(screen.getByText("BPC-157 Stack")).toBeDefined();
    expect(screen.getByLabelText("Start Date")).toBeDefined();
    expect(screen.getByRole("button", { name: "Start Run" })).toBeDefined();
    expect(screen.getByText("Cancel")).toBeDefined();
  });

  it("defaults start date to today", () => {
    server.use(http.post("/api/v1/protocols/:id/runs", () => new Promise(() => {})));

    renderModal();

    const dateInput = screen.getByLabelText("Start Date") as HTMLInputElement;
    const today = new Date();
    const expected = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, "0")}-${String(today.getDate()).padStart(2, "0")}`;
    expect(dateInput.value).toBe(expected);
  });

  it("calls onClose when Cancel is clicked", async () => {
    server.use(http.post("/api/v1/protocols/:id/runs", () => new Promise(() => {})));

    const { onClose } = renderModal();
    const user = userEvent.setup();

    await user.click(screen.getByText("Cancel"));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("submits start run request on form submit", async () => {
    let capturedBody: unknown;

    server.use(
      http.post("/api/v1/protocols/:id/runs", async ({ request }) => {
        capturedBody = await request.json();
        return HttpResponse.json({
          id: "run-1",
          protocol_id: "proto-1",
          user_id: "user-1",
          start_date: "2026-03-28",
          status: "active",
          notify: false,
          notify_times: [],
          repeat_reminders: false,
          repeat_interval_minutes: 30,
          created_at: "2026-03-28T10:00:00Z",
        });
      }),
    );

    const { onClose, onStarted } = renderModal();
    const user = userEvent.setup();

    await user.click(screen.getByText("Start Run", { selector: "button[type='submit']" }));

    await waitFor(() => {
      expect(onClose).toHaveBeenCalled();
    });
    expect(onStarted).toHaveBeenCalled();
    expect(capturedBody).toMatchObject({
      notify: false,
    });
  });

  it("shows notification options when checkbox is checked", async () => {
    server.use(http.post("/api/v1/protocols/:id/runs", () => new Promise(() => {})));

    renderModal();
    const user = userEvent.setup();

    // Initially no notification times shown
    expect(screen.queryByText("Notification Times")).toBeNull();

    await user.click(screen.getByLabelText("Enable notifications"));

    expect(screen.getByText("Notification Times")).toBeDefined();
    expect(screen.getByText("Repeat if not logged (every 30 min)")).toBeDefined();
  });

  it("shows error on API failure", async () => {
    server.use(
      http.post("/api/v1/protocols/:id/runs", () => new HttpResponse("Conflict", { status: 409 })),
    );

    renderModal();
    const user = userEvent.setup();

    await user.click(screen.getByText("Start Run", { selector: "button[type='submit']" }));

    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeDefined();
    });
    expect(screen.getByRole("alert").textContent).toContain("Failed to start run");
  });

  it("calls onClose when overlay is clicked", async () => {
    server.use(http.post("/api/v1/protocols/:id/runs", () => new Promise(() => {})));

    const { onClose } = renderModal();
    const user = userEvent.setup();

    // Click on the overlay (the presentation role element)
    const overlay = screen.getByRole("presentation");
    await user.click(overlay);
    expect(onClose).toHaveBeenCalled();
  });
});
