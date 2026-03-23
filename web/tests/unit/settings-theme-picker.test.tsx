// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { useAuthStore } from "../../src/store/auth";

const TOKEN = "test-jwt";

const server = setupServer(
  http.get("/api/v1/source-preferences", () => HttpResponse.json([])),
  http.get("/api/v1/auth/methods", () => HttpResponse.json([])),
);

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

function wrapper({ children }: { children: React.ReactNode }) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
}

async function renderSettings() {
  const { default: Settings } = await import("../../src/pages/Settings");
  return render(<Settings />, { wrapper });
}

describe("Settings — Theme Picker", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: TOKEN, isAuthenticated: true });
    localStorage.clear();
    delete document.documentElement.dataset.theme;
  });

  it("renders three theme options", async () => {
    await renderSettings();

    const group = screen.getByRole("radiogroup", { name: /theme/i });
    expect(group).toBeDefined();

    const options = screen.getAllByRole("radio");
    expect(options).toHaveLength(3);
    expect(screen.getByText("Light")).toBeDefined();
    expect(screen.getByText("Dark")).toBeDefined();
    expect(screen.getByText("System")).toBeDefined();
  });

  it("defaults to system theme", async () => {
    await renderSettings();

    expect(screen.getByText("System")).toHaveAttribute("aria-checked", "true");
    expect(screen.getByText("Light")).toHaveAttribute("aria-checked", "false");
    expect(screen.getByText("Dark")).toHaveAttribute("aria-checked", "false");
  });

  it("selects dark theme on click", async () => {
    await renderSettings();
    const user = userEvent.setup();

    await user.click(screen.getByText("Dark"));

    expect(screen.getByText("Dark")).toHaveAttribute("aria-checked", "true");
    expect(screen.getByText("Light")).toHaveAttribute("aria-checked", "false");
    expect(screen.getByText("System")).toHaveAttribute("aria-checked", "false");
    expect(localStorage.getItem("theme")).toBe("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("selects light theme on click", async () => {
    await renderSettings();
    const user = userEvent.setup();

    await user.click(screen.getByText("Light"));

    expect(screen.getByText("Light")).toHaveAttribute("aria-checked", "true");
    expect(localStorage.getItem("theme")).toBe("light");
    expect(document.documentElement.dataset.theme).toBe("light");
  });

  it("clears localStorage when switching back to system", async () => {
    localStorage.setItem("theme", "dark");
    await renderSettings();
    const user = userEvent.setup();

    await user.click(screen.getByText("System"));

    expect(screen.getByText("System")).toHaveAttribute("aria-checked", "true");
    expect(localStorage.getItem("theme")).toBeNull();
    expect(document.documentElement.dataset.theme).toBeUndefined();
  });

  it("reflects stored theme on initial render", async () => {
    localStorage.setItem("theme", "dark");
    await renderSettings();

    expect(screen.getByText("Dark")).toHaveAttribute("aria-checked", "true");
    expect(screen.getByText("Light")).toHaveAttribute("aria-checked", "false");
    expect(screen.getByText("System")).toHaveAttribute("aria-checked", "false");
  });
});
