// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { MemoryRouter } from "react-router-dom";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { ImportModal } from "../../src/components/protocols/ImportModal";
import { useAuthStore } from "../../src/store/auth";

const server = setupServer();

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

const exportJson = JSON.stringify({
  schema: "ownpulse-protocol/v1",
  name: "BPC-157 Stack",
  duration_days: 14,
  tags: [],
  lines: [{ substance: "BPC-157", dose: 250, unit: "mcg", pattern: "daily" }],
});

function renderModal(onClose: () => void = vi.fn()) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <ImportModal onClose={onClose} />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

function makeFile(content: string) {
  return new File([content], "protocol.json", { type: "application/json" });
}

describe("ImportModal", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: "test-jwt", isAuthenticated: true });
  });

  it("renders the dropzone", () => {
    renderModal();
    expect(screen.getByText("Import Protocol")).toBeDefined();
    expect(screen.getByText("Drop a .json protocol file here")).toBeDefined();
  });

  it("previews a valid parsed file and imports it against /api/v1/protocols/import", async () => {
    let capturedUrl: string | undefined;
    server.use(
      http.post("/api/v1/protocols/import", async ({ request }) => {
        capturedUrl = request.url;
        return HttpResponse.json({ id: "proto-new" }, { status: 201 });
      }),
    );

    const onClose = vi.fn();
    renderModal(onClose);
    const user = userEvent.setup();

    const input = document.querySelector('input[type="file"]') as HTMLInputElement;
    await user.upload(input, makeFile(exportJson));

    await waitFor(() => {
      expect(screen.getByText("BPC-157 Stack")).toBeDefined();
    });

    const importButton = screen.getByRole("button", { name: "Import" });
    expect(importButton).not.toBeDisabled();
    await user.click(importButton);

    await waitFor(() => {
      expect(onClose).toHaveBeenCalled();
    });

    expect(capturedUrl).toContain("/api/v1/protocols/import");
    expect(capturedUrl).not.toContain("import-file");
  });

  it("shows an error message for invalid JSON", async () => {
    renderModal();
    const user = userEvent.setup();

    const input = document.querySelector('input[type="file"]') as HTMLInputElement;
    await user.upload(input, makeFile("not json"));

    await waitFor(() => {
      expect(screen.getByText("Invalid JSON file.")).toBeDefined();
    });
    expect(screen.getByRole("button", { name: "Import" })).toBeDisabled();
  });

  it("shows an error message when the name is missing", async () => {
    renderModal();
    const user = userEvent.setup();

    const input = document.querySelector('input[type="file"]') as HTMLInputElement;
    await user.upload(input, makeFile(JSON.stringify({ schema: "ownpulse-protocol/v1" })));

    await waitFor(() => {
      expect(screen.getByText("Invalid protocol file: missing a name.")).toBeDefined();
    });
    expect(screen.getByRole("button", { name: "Import" })).toBeDisabled();
  });

  it("shows an error message for an unsupported schema", async () => {
    renderModal();
    const user = userEvent.setup();

    const input = document.querySelector('input[type="file"]') as HTMLInputElement;
    await user.upload(
      input,
      makeFile(JSON.stringify({ ...JSON.parse(exportJson), schema: "some-other-schema/v9" })),
    );

    await waitFor(() => {
      expect(
        screen.getByText('Unsupported protocol file: expected schema "ownpulse-protocol/v1".'),
      ).toBeDefined();
    });
  });

  it("shows an error message when tags is missing (backend requires the key)", async () => {
    renderModal();
    const user = userEvent.setup();

    const withoutTags = JSON.parse(exportJson);
    delete withoutTags.tags;

    const input = document.querySelector('input[type="file"]') as HTMLInputElement;
    await user.upload(input, makeFile(JSON.stringify(withoutTags)));

    await waitFor(() => {
      expect(screen.getByText("Invalid protocol file: missing tags.")).toBeDefined();
    });
  });

  it("shows an error message when a line is missing its substance", async () => {
    renderModal();
    const user = userEvent.setup();

    const badLine = JSON.parse(exportJson);
    badLine.lines = [{ dose: 250, unit: "mcg", pattern: "daily" }];

    const input = document.querySelector('input[type="file"]') as HTMLInputElement;
    await user.upload(input, makeFile(JSON.stringify(badLine)));

    await waitFor(() => {
      expect(
        screen.getByText("Invalid protocol file: every line needs a substance."),
      ).toBeDefined();
    });
  });

  it("shows an error message when a line is missing its schedule pattern", async () => {
    renderModal();
    const user = userEvent.setup();

    const badLine = JSON.parse(exportJson);
    badLine.lines = [{ substance: "BPC-157", dose: 250, unit: "mcg" }];

    const input = document.querySelector('input[type="file"]') as HTMLInputElement;
    await user.upload(input, makeFile(JSON.stringify(badLine)));

    await waitFor(() => {
      expect(
        screen.getByText("Invalid protocol file: every line needs a schedule pattern."),
      ).toBeDefined();
    });
  });

  it("shows a friendly, non-raw error message when the import request fails", async () => {
    server.use(
      http.post(
        "/api/v1/protocols/import",
        () => new HttpResponse("duplicate key value violates unique constraint", { status: 500 }),
      ),
    );

    renderModal();
    const user = userEvent.setup();

    const input = document.querySelector('input[type="file"]') as HTMLInputElement;
    await user.upload(input, makeFile(exportJson));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Import" })).not.toBeDisabled();
    });

    await user.click(screen.getByRole("button", { name: "Import" }));

    await waitFor(() => {
      expect(screen.getByText("Import failed. Please check the file and try again.")).toBeDefined();
    });
    // The raw backend error text must never reach the DOM.
    expect(screen.queryByText(/unique constraint/)).toBeNull();
  });

  it("calls onClose when Cancel is clicked", async () => {
    const onClose = vi.fn();
    renderModal(onClose);
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onClose).toHaveBeenCalled();
  });
});
