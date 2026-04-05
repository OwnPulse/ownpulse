// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { FeatureFlagsSection } from "../../src/components/admin/FeatureFlags";
import { useAuthStore } from "../../src/store/auth";

const TOKEN = "test-jwt";

const mockFlags = [
  {
    id: "f1",
    key: "dark_mode_v2",
    enabled: true,
    description: "Enable dark mode v2",
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
  },
  {
    id: "f2",
    key: "new_dashboard",
    enabled: false,
    description: null,
    created_at: "2026-02-01T00:00:00Z",
    updated_at: "2026-02-01T00:00:00Z",
  },
];

const server = setupServer();

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

function renderComponent() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <FeatureFlagsSection />
    </QueryClientProvider>,
  );
}

describe("FeatureFlagsSection", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: TOKEN, isAuthenticated: true, role: "admin" });
  });

  it("renders list of flags", async () => {
    server.use(http.get("/api/v1/admin/feature-flags", () => HttpResponse.json(mockFlags)));

    renderComponent();

    await waitFor(() => {
      expect(screen.getByText("dark_mode_v2")).toBeDefined();
    });

    expect(screen.getByText("new_dashboard")).toBeDefined();
    expect(screen.getByText("Enable dark mode v2")).toBeDefined();
  });

  it("renders loading state", () => {
    server.use(
      http.get("/api/v1/admin/feature-flags", async () => {
        await new Promise(() => {});
        return HttpResponse.json(mockFlags);
      }),
    );

    renderComponent();

    expect(screen.getByText("Loading feature flags...")).toBeDefined();
  });

  it("renders error state", async () => {
    server.use(
      http.get(
        "/api/v1/admin/feature-flags",
        () => new HttpResponse("Server Error", { status: 500 }),
      ),
    );

    renderComponent();

    await waitFor(() => {
      expect(screen.getByText("Error loading feature flags.")).toBeDefined();
    });
  });

  it("renders empty state", async () => {
    server.use(http.get("/api/v1/admin/feature-flags", () => HttpResponse.json([])));

    renderComponent();

    await waitFor(() => {
      expect(screen.getByText("No feature flags yet.")).toBeDefined();
    });
  });

  it("toggles a flag via PUT", async () => {
    const user = userEvent.setup();
    let capturedBody: unknown;
    let capturedKey: string | undefined;

    server.use(
      http.get("/api/v1/admin/feature-flags", () => HttpResponse.json(mockFlags)),
      http.put("/api/v1/admin/feature-flags/:key", async ({ params, request }) => {
        capturedKey = params.key as string;
        capturedBody = await request.json();
        return HttpResponse.json({
          ...mockFlags[0],
          enabled: false,
        });
      }),
    );

    renderComponent();

    await waitFor(() => {
      expect(screen.getByText("dark_mode_v2")).toBeDefined();
    });

    const toggle = screen.getByTestId("toggle-dark_mode_v2");
    await user.click(toggle);

    await waitFor(() => {
      expect(capturedKey).toBe("dark_mode_v2");
      expect(capturedBody).toEqual({ enabled: false });
    });
  });

  it("creates a new flag via the form", async () => {
    const user = userEvent.setup();
    let capturedKey: string | undefined;
    let capturedBody: unknown;

    server.use(
      http.get("/api/v1/admin/feature-flags", () => HttpResponse.json(mockFlags)),
      http.put("/api/v1/admin/feature-flags/:key", async ({ params, request }) => {
        capturedKey = params.key as string;
        capturedBody = await request.json();
        return HttpResponse.json({
          id: "f3",
          key: "beta_feature",
          enabled: true,
          description: "A beta feature",
          created_at: "2026-03-01T00:00:00Z",
          updated_at: "2026-03-01T00:00:00Z",
        });
      }),
    );

    renderComponent();

    await waitFor(() => {
      expect(screen.getByText("dark_mode_v2")).toBeDefined();
    });

    // Open the form
    await user.click(screen.getByText("New Flag"));

    // Fill in the form
    const keyInput = screen.getByLabelText("Key");
    const descInput = screen.getByLabelText("Description");

    await user.type(keyInput, "beta_feature");
    await user.type(descInput, "A beta feature");

    // Toggle enabled
    const enabledToggle = screen.getByTestId("new-flag-enabled-toggle");
    await user.click(enabledToggle);

    // Submit
    await user.click(screen.getByText("Create"));

    await waitFor(() => {
      expect(capturedKey).toBe("beta_feature");
      expect(capturedBody).toEqual({
        enabled: true,
        description: "A beta feature",
      });
    });
  });

  it("deletes a flag after confirmation", async () => {
    const user = userEvent.setup();
    let capturedKey: string | undefined;

    vi.spyOn(window, "confirm").mockReturnValue(true);

    server.use(
      http.get("/api/v1/admin/feature-flags", () => HttpResponse.json(mockFlags)),
      http.delete("/api/v1/admin/feature-flags/:key", ({ params }) => {
        capturedKey = params.key as string;
        return new HttpResponse(null, { status: 204 });
      }),
    );

    renderComponent();

    await waitFor(() => {
      expect(screen.getByText("dark_mode_v2")).toBeDefined();
    });

    const deleteButtons = screen.getAllByText("Delete");
    await user.click(deleteButtons[0]);

    await waitFor(() => {
      expect(capturedKey).toBe("dark_mode_v2");
    });

    expect(window.confirm).toHaveBeenCalledWith(
      'Delete flag "dark_mode_v2"? This cannot be undone.',
    );

    vi.restoreAllMocks();
  });

  it("does not delete a flag when confirmation is cancelled", async () => {
    const user = userEvent.setup();
    let deleteCalled = false;

    vi.spyOn(window, "confirm").mockReturnValue(false);

    server.use(
      http.get("/api/v1/admin/feature-flags", () => HttpResponse.json(mockFlags)),
      http.delete("/api/v1/admin/feature-flags/:key", () => {
        deleteCalled = true;
        return new HttpResponse(null, { status: 204 });
      }),
    );

    renderComponent();

    await waitFor(() => {
      expect(screen.getByText("dark_mode_v2")).toBeDefined();
    });

    const deleteButtons = screen.getAllByText("Delete");
    await user.click(deleteButtons[0]);

    expect(deleteCalled).toBe(false);

    vi.restoreAllMocks();
  });
});
