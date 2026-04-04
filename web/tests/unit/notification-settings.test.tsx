// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import NotificationSettings from "../../src/components/settings/NotificationSettings";
import { useAuthStore } from "../../src/store/auth";

const defaultPrefs = {
  default_notify: false,
  default_notify_times: ["08:00"],
  repeat_reminders: false,
  repeat_interval_minutes: 30,
};

const server = setupServer(
  http.get("/api/v1/notifications/preferences", () => {
    return HttpResponse.json(defaultPrefs);
  }),
  http.put("/api/v1/notifications/preferences", async ({ request }) => {
    const body = await request.json();
    return HttpResponse.json(body);
  }),
);

beforeAll(() => server.listen());
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

function renderComponent() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <NotificationSettings />
    </QueryClientProvider>,
  );
}

describe("NotificationSettings", () => {
  beforeEach(() => {
    useAuthStore.getState().login("test-jwt-token");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  it("renders loading state", () => {
    renderComponent();
    expect(screen.getByText("Loading...")).toBeDefined();
  });

  it("renders with data after loading", async () => {
    renderComponent();
    await waitFor(() => {
      expect(screen.getByLabelText(/enable notifications for new protocol runs/i)).toBeDefined();
    });
    expect(screen.getByRole("button", { name: /save notification settings/i })).toBeDefined();
  });

  it("renders error state", async () => {
    server.use(
      http.get("/api/v1/notifications/preferences", () => {
        return new HttpResponse("Internal Server Error", { status: 500 });
      }),
    );
    renderComponent();
    await waitFor(() => {
      expect(screen.getByText(/error loading notification preferences/i)).toBeDefined();
    });
  });

  it("shows time pickers and repeat settings when notify is enabled", async () => {
    server.use(
      http.get("/api/v1/notifications/preferences", () => {
        return HttpResponse.json({
          default_notify: true,
          default_notify_times: ["08:00"],
          repeat_reminders: false,
          repeat_interval_minutes: 30,
        });
      }),
    );

    renderComponent();
    await waitFor(() => {
      expect(screen.getByLabelText(/notification time 1/i)).toBeDefined();
    });
    expect(screen.getByLabelText(/repeat reminders if dose not logged/i)).toBeDefined();
  });

  it("toggles notification enable checkbox", async () => {
    const user = userEvent.setup();
    renderComponent();

    await waitFor(() => {
      expect(screen.getByLabelText(/enable notifications for new protocol runs/i)).toBeDefined();
    });

    const checkbox = screen.getByLabelText(
      /enable notifications for new protocol runs/i,
    ) as HTMLInputElement;
    expect(checkbox.checked).toBe(false);

    await user.click(checkbox);
    expect(checkbox.checked).toBe(true);

    // Time picker should now appear
    await waitFor(() => {
      expect(screen.getByLabelText(/notification time 1/i)).toBeDefined();
    });
  });

  it("adds and removes notification times", async () => {
    server.use(
      http.get("/api/v1/notifications/preferences", () => {
        return HttpResponse.json({
          default_notify: true,
          default_notify_times: ["08:00"],
          repeat_reminders: false,
          repeat_interval_minutes: 30,
        });
      }),
    );

    const user = userEvent.setup();
    renderComponent();

    await waitFor(() => {
      expect(screen.getByLabelText(/notification time 1/i)).toBeDefined();
    });

    // Add a second time
    await user.click(screen.getByRole("button", { name: /add time/i }));
    expect(screen.getByLabelText(/notification time 2/i)).toBeDefined();

    // Now both have remove buttons
    const removeButtons = screen.getAllByRole("button", { name: /remove time/i });
    expect(removeButtons).toHaveLength(2);

    // Remove the first one
    await user.click(removeButtons[0]);
    expect(screen.queryByLabelText(/notification time 2/i)).toBeNull();
  });

  it("saves preferences successfully", async () => {
    const user = userEvent.setup();
    renderComponent();

    await waitFor(() => {
      expect(screen.getByLabelText(/enable notifications for new protocol runs/i)).toBeDefined();
    });

    await user.click(screen.getByRole("button", { name: /save notification settings/i }));

    await waitFor(() => {
      expect(screen.getByText("Preferences saved!")).toBeDefined();
    });
  });

  it("shows error on save failure", async () => {
    server.use(
      http.put("/api/v1/notifications/preferences", () => {
        return new HttpResponse("Bad Request", { status: 400 });
      }),
    );

    const user = userEvent.setup();
    renderComponent();

    await waitFor(() => {
      expect(screen.getByLabelText(/enable notifications for new protocol runs/i)).toBeDefined();
    });

    await user.click(screen.getByRole("button", { name: /save notification settings/i }));

    await waitFor(() => {
      expect(screen.getByText(/error:/i)).toBeDefined();
    });
  });

  it("shows repeat interval field when repeat reminders enabled", async () => {
    server.use(
      http.get("/api/v1/notifications/preferences", () => {
        return HttpResponse.json({
          default_notify: true,
          default_notify_times: ["08:00"],
          repeat_reminders: true,
          repeat_interval_minutes: 15,
        });
      }),
    );

    renderComponent();
    await waitFor(() => {
      expect(screen.getByLabelText(/repeat interval/i)).toBeDefined();
    });
    expect(screen.getByLabelText(/repeat interval/i)).toHaveValue(15);
  });
});
