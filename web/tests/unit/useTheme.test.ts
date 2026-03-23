// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { useTheme } from "../../src/hooks/useTheme";

function makeMatchMedia(darkMode: boolean) {
  return (query: string) => ({
    matches: darkMode && query === "(prefers-color-scheme: dark)",
    media: query,
    onchange: null,
    addListener: () => {},
    removeListener: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
    dispatchEvent: () => false,
  });
}

describe("useTheme", () => {
  beforeEach(() => {
    localStorage.clear();
    delete document.documentElement.dataset.theme;
  });

  it("defaults to system theme when localStorage is empty", () => {
    const { result } = renderHook(() => useTheme());

    expect(result.current.theme).toBe("system");
    // matchMedia stub returns matches: false, so system resolves to light
    expect(result.current.resolvedTheme).toBe("light");
  });

  it("initializes from stored localStorage value", () => {
    localStorage.setItem("theme", "dark");
    const { result } = renderHook(() => useTheme());

    expect(result.current.theme).toBe("dark");
    expect(result.current.resolvedTheme).toBe("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("setTheme dark writes localStorage and sets data-theme", () => {
    const { result } = renderHook(() => useTheme());

    act(() => result.current.setTheme("dark"));

    expect(result.current.theme).toBe("dark");
    expect(result.current.resolvedTheme).toBe("dark");
    expect(localStorage.getItem("theme")).toBe("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("setTheme light writes localStorage and sets data-theme", () => {
    const { result } = renderHook(() => useTheme());

    act(() => result.current.setTheme("light"));

    expect(result.current.theme).toBe("light");
    expect(result.current.resolvedTheme).toBe("light");
    expect(localStorage.getItem("theme")).toBe("light");
    expect(document.documentElement.dataset.theme).toBe("light");
  });

  it("setTheme system removes localStorage and data-theme attribute", () => {
    localStorage.setItem("theme", "dark");
    const { result } = renderHook(() => useTheme());

    act(() => result.current.setTheme("system"));

    expect(result.current.theme).toBe("system");
    expect(localStorage.getItem("theme")).toBeNull();
    expect(document.documentElement.dataset.theme).toBeUndefined();
  });

  describe("with dark system preference", () => {
    const originalMatchMedia = window.matchMedia;

    beforeEach(() => {
      Object.defineProperty(window, "matchMedia", {
        writable: true,
        value: makeMatchMedia(true),
      });
    });

    afterEach(() => {
      Object.defineProperty(window, "matchMedia", {
        writable: true,
        value: originalMatchMedia,
      });
    });

    it("resolvedTheme reflects dark system preference when theme is system", () => {
      const { result } = renderHook(() => useTheme());

      expect(result.current.theme).toBe("system");
      expect(result.current.resolvedTheme).toBe("dark");
    });
  });

  it("treats invalid localStorage values as system", () => {
    localStorage.setItem("theme", "garbage");
    const { result } = renderHook(() => useTheme());

    expect(result.current.theme).toBe("system");
    expect(result.current.resolvedTheme).toBe("light");
  });
});
