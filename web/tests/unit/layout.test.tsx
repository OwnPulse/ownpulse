// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import Layout from "../../src/components/Layout";
import { useAuthStore } from "../../src/store/auth";

// Layout renders <Outlet /> which needs a matching route; keep it minimal
// since these tests only cover the sidebar/nav chrome, not routed content.
function renderLayout() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={["/"]}>
        <Layout />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("Layout", () => {
  beforeEach(() => {
    // No token: useSSE() early-returns instead of opening a real EventSource,
    // which jsdom doesn't implement.
    useAuthStore.setState({ token: null, isAuthenticated: false, role: null });
    // Keep the theme-cycle test deterministic regardless of test order.
    localStorage.removeItem("theme");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  it("renders the main nav with an accessible label and the Log entry", () => {
    renderLayout();

    const nav = screen.getByRole("navigation", { name: "Main" });
    expect(nav).toBeDefined();
    expect(screen.getByRole("link", { name: "Log" })).toBeDefined();
    expect(screen.getByRole("link", { name: "Dashboard" })).toBeDefined();
    expect(screen.getByRole("link", { name: "Sources" })).toBeDefined();
  });

  it("does not render the Admin nav link for a non-admin user", () => {
    renderLayout();
    expect(screen.queryByRole("link", { name: "Admin" })).toBeNull();
  });

  it("renders the Admin nav link for an admin user", () => {
    useAuthStore.setState({ token: null, isAuthenticated: false, role: "admin" });
    renderLayout();
    expect(screen.getByRole("link", { name: "Admin" })).toBeDefined();
  });

  // The overlay button is also labeled "Close menu" (unconditionally), so once
  // the sidebar is open there are two "Close menu" buttons. Only the
  // hamburger carries `aria-expanded` — use that to disambiguate it from the
  // overlay.
  function getHamburger(name: "Open menu" | "Close menu") {
    const btn = screen
      .getAllByRole("button", { name })
      .find((b) => b.hasAttribute("aria-expanded"));
    if (!btn) throw new Error(`hamburger button "${name}" not found`);
    return btn;
  }

  it("toggles the hamburger button's aria-expanded and label when clicked", async () => {
    renderLayout();
    const user = userEvent.setup();

    const menuBtn = getHamburger("Open menu");
    expect(menuBtn.getAttribute("aria-expanded")).toBe("false");

    await user.click(menuBtn);
    const closeBtn = getHamburger("Close menu");
    expect(closeBtn.getAttribute("aria-expanded")).toBe("true");

    await user.click(closeBtn);
    expect(getHamburger("Open menu").getAttribute("aria-expanded")).toBe("false");
  });

  it("closes the open sidebar on Escape", async () => {
    renderLayout();
    const user = userEvent.setup();

    await user.click(getHamburger("Open menu"));
    expect(getHamburger("Close menu").getAttribute("aria-expanded")).toBe("true");

    await user.keyboard("{Escape}");
    expect(getHamburger("Open menu").getAttribute("aria-expanded")).toBe("false");
  });

  it("closes the open sidebar when the overlay is clicked", async () => {
    renderLayout();
    const user = userEvent.setup();

    await user.click(getHamburger("Open menu"));
    expect(getHamburger("Close menu").getAttribute("aria-expanded")).toBe("true");

    // The overlay is the "Close menu" button *without* aria-expanded.
    const overlay = screen
      .getAllByRole("button", { name: "Close menu" })
      .find((btn) => !btn.hasAttribute("aria-expanded"));
    if (!overlay) throw new Error("overlay button not found");
    await user.click(overlay);

    expect(getHamburger("Open menu").getAttribute("aria-expanded")).toBe("false");
  });

  it("cycles the theme toggle label through light -> dark -> system -> light", async () => {
    renderLayout();
    const user = userEvent.setup();

    const themeBtn = () => screen.getByRole("button", { name: /^Theme:/ });
    const initialLabel = themeBtn().textContent;
    expect(initialLabel).toMatch(/^Theme: (Light|Dark|System)$/);

    await user.click(themeBtn());
    const afterFirstClick = themeBtn().textContent;
    expect(afterFirstClick).not.toBe(initialLabel);

    await user.click(themeBtn());
    const afterSecondClick = themeBtn().textContent;
    expect(afterSecondClick).not.toBe(afterFirstClick);

    await user.click(themeBtn());
    // Back to the starting label after a full cycle of three clicks.
    expect(themeBtn().textContent).toBe(initialLabel);
  });
});
