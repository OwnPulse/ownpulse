// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAuthStore } from "../../src/store/auth";

describe("protocolsApi", () => {
  let mockFetch: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    useAuthStore.setState({ token: "test-jwt", isAuthenticated: true });
    vi.restoreAllMocks();

    mockFetch = vi.fn();
    vi.stubGlobal("fetch", mockFetch);
  });

  it("create sends correct payload", async () => {
    const createdProtocol = {
      id: "p1",
      name: "Test Protocol",
      start_date: "2026-04-01",
      duration_days: 14,
      status: "active",
      lines: [],
      created_at: "2026-03-27T00:00:00Z",
      updated_at: "2026-03-27T00:00:00Z",
    };

    mockFetch.mockResolvedValue({
      ok: true,
      status: 200,
      json: () => Promise.resolve(createdProtocol),
    });

    const { protocolsApi } = await import("../../src/api/protocols");

    const payload = {
      name: "Test Protocol",
      start_date: "2026-04-01",
      duration_days: 14,
      lines: [
        {
          substance: "Vitamin D",
          dose: 5000,
          unit: "IU",
          schedule_pattern: [true, true, true, true, true, true, true],
          sort_order: 0,
        },
      ],
    };

    const result = await protocolsApi.create(payload);

    expect(result).toEqual(createdProtocol);
    expect(mockFetch).toHaveBeenCalledOnce();

    const [url, options] = mockFetch.mock.calls[0];
    expect(url).toBe("/api/v1/protocols");
    expect(options.method).toBe("POST");
    expect(JSON.parse(options.body)).toEqual(payload);

    vi.unstubAllGlobals();
  });

  it("exportProtocol returns protocol data", async () => {
    const exportData = {
      schema: "ownpulse/protocol/v1",
      name: "Exported Protocol",
      duration_days: 7,
      tags: ["sleep"],
      lines: [{ substance: "Melatonin", dose: 3, unit: "mg", pattern: "daily" }],
    };

    mockFetch.mockResolvedValue({
      ok: true,
      status: 200,
      json: () => Promise.resolve(exportData),
    });

    const { protocolsApi } = await import("../../src/api/protocols");
    const result = await protocolsApi.exportProtocol("p1");

    expect(result).toEqual(exportData);

    const [url] = mockFetch.mock.calls[0];
    expect(url).toBe("/api/v1/protocols/p1/export");

    vi.unstubAllGlobals();
  });

  it("listTemplates returns templates", async () => {
    const templates = [
      {
        id: "tpl-1",
        name: "Basic Stack",
        description: "A starter stack",
        tags: ["beginner"],
        duration_days: 30,
        line_count: 2,
      },
      {
        id: "tpl-2",
        name: "Advanced Stack",
        description: null,
        tags: ["advanced", "nootropic"],
        duration_days: 60,
        line_count: 5,
      },
    ];

    mockFetch.mockResolvedValue({
      ok: true,
      status: 200,
      json: () => Promise.resolve(templates),
    });

    const { protocolsApi } = await import("../../src/api/protocols");
    const result = await protocolsApi.listTemplates();

    expect(result).toEqual(templates);

    const [url] = mockFetch.mock.calls[0];
    expect(url).toBe("/api/v1/protocols/templates");

    vi.unstubAllGlobals();
  });
});
