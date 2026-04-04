// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { notificationsApi } from "../../src/api/notifications";
import { useAuthStore } from "../../src/store/auth";

const defaultPrefs = {
  default_notify: true,
  default_notify_times: ["08:00", "20:00"],
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

describe("notificationsApi", () => {
  beforeEach(() => {
    useAuthStore.getState().login("test-jwt-token");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  describe("getPreferences", () => {
    it("fetches notification preferences", async () => {
      const result = await notificationsApi.getPreferences();
      expect(result.default_notify).toBe(true);
      expect(result.default_notify_times).toEqual(["08:00", "20:00"]);
      expect(result.repeat_reminders).toBe(false);
      expect(result.repeat_interval_minutes).toBe(30);
    });

    it("handles 401 error", async () => {
      server.use(
        http.get("/api/v1/notifications/preferences", () => {
          return new HttpResponse("Unauthorized", { status: 401 });
        }),
      );
      await expect(notificationsApi.getPreferences()).rejects.toThrow("Unauthorized");
    });

    it("handles 403 error", async () => {
      server.use(
        http.get("/api/v1/notifications/preferences", () => {
          return new HttpResponse("Forbidden", { status: 403 });
        }),
      );
      await expect(notificationsApi.getPreferences()).rejects.toThrow("Forbidden");
    });

    it("handles 500 error", async () => {
      server.use(
        http.get("/api/v1/notifications/preferences", () => {
          return new HttpResponse("Internal Server Error", { status: 500 });
        }),
      );
      await expect(notificationsApi.getPreferences()).rejects.toThrow("Internal Server Error");
    });
  });

  describe("updatePreferences", () => {
    it("updates notification preferences", async () => {
      const updated = {
        default_notify: true,
        default_notify_times: ["09:00"],
        repeat_reminders: true,
        repeat_interval_minutes: 15,
      };
      const result = await notificationsApi.updatePreferences(updated);
      expect(result.default_notify).toBe(true);
      expect(result.default_notify_times).toEqual(["09:00"]);
      expect(result.repeat_reminders).toBe(true);
      expect(result.repeat_interval_minutes).toBe(15);
    });

    it("handles 401 error", async () => {
      server.use(
        http.put("/api/v1/notifications/preferences", () => {
          return new HttpResponse("Unauthorized", { status: 401 });
        }),
      );
      await expect(
        notificationsApi.updatePreferences({
          default_notify: false,
          default_notify_times: [],
          repeat_reminders: false,
          repeat_interval_minutes: 30,
        }),
      ).rejects.toThrow("Unauthorized");
    });

    it("handles 500 error", async () => {
      server.use(
        http.put("/api/v1/notifications/preferences", () => {
          return new HttpResponse("Internal Server Error", { status: 500 });
        }),
      );
      await expect(
        notificationsApi.updatePreferences({
          default_notify: false,
          default_notify_times: [],
          repeat_reminders: false,
          repeat_interval_minutes: 30,
        }),
      ).rejects.toThrow("Internal Server Error");
    });
  });
});
