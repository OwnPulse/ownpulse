// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import "@testing-library/jest-dom/vitest";

// Node's built-in `localStorage`/`sessionStorage` globals (stable since
// Node 22) already exist on globalThis before vitest's jsdom environment
// populates it. Vitest's global-population step skips any key that's
// already present on globalThis, so jsdom's real Storage implementation
// never gets copied over — bare `localStorage`/`sessionStorage` in tests
// resolve to Node's own (non-functional without `--localstorage-file`)
// storage instead, throwing on every call. jsdom's actual window is still
// reachable internally via `globalThis.jsdom.window`; grab the real
// Storage instances from there and force globalThis to point at them
// before any test touches storage. Falls back to `window[key]` (a no-op
// re-assignment) when that internal isn't present, so this stays safe
// across vitest/jsdom versions and Node releases where the conflict
// doesn't occur.
const jsdomWindow = (globalThis as unknown as { jsdom?: { window?: Window } }).jsdom?.window;
for (const key of ["localStorage", "sessionStorage"] as const) {
  const real = jsdomWindow?.[key] ?? window[key];
  if (real) {
    Object.defineProperty(globalThis, key, {
      value: real,
      configurable: true,
      writable: true,
    });
  }
}

// jsdom does not implement window.matchMedia — stub it for tests that use it
// (e.g., useTheme hook, any component that checks prefers-color-scheme).
Object.defineProperty(window, "matchMedia", {
  writable: true,
  value: (query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: () => {},
    removeListener: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
    dispatchEvent: () => false,
  }),
});
