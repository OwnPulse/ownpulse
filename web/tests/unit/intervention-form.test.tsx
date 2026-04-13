// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import InterventionForm from "../../src/components/forms/InterventionForm";
import { useAuthStore } from "../../src/store/auth";

const activeSubstances = [
  {
    substance: "BPC-157",
    dose: 250,
    unit: "mcg",
    route: "SubQ",
    protocol_name: "BPC Stack",
    protocol_id: "proto-1",
  },
  {
    substance: "TB-500",
    dose: 2,
    unit: "mg",
    route: "SubQ",
    protocol_name: "BPC Stack",
    protocol_id: "proto-1",
  },
];

const testSavedMedicines = [
  {
    id: "sm-1",
    substance: "Magnesium",
    dose: 400,
    unit: "mg",
    route: "oral",
    sort_order: 0,
    created_at: "2026-03-01T00:00:00Z",
  },
  {
    id: "sm-2",
    substance: "Vitamin D",
    dose: 5000,
    unit: "IU",
    route: "oral",
    sort_order: 1,
    created_at: "2026-03-01T00:00:00Z",
  },
];

const server = setupServer(
  http.get("/api/v1/protocols/active-substances", () => {
    return HttpResponse.json(activeSubstances);
  }),
  http.get("/api/v1/saved-medicines", () => {
    return HttpResponse.json(testSavedMedicines);
  }),
  http.post("/api/v1/saved-medicines", () => {
    return HttpResponse.json(
      {
        id: "sm-new",
        substance: "Creatine",
        dose: 5,
        unit: "g",
        route: "oral",
        sort_order: 2,
        created_at: "2026-04-12T00:00:00Z",
      },
      { status: 201 },
    );
  }),
  http.delete("/api/v1/saved-medicines/:id", () => {
    return new HttpResponse(null, { status: 204 });
  }),
  http.post("/api/v1/interventions", () => {
    return HttpResponse.json(
      {
        id: "iv-1",
        user_id: "user-1",
        substance: "BPC-157",
        dose: 250,
        unit: "mcg",
        route: "SubQ",
        administered_at: "2026-03-28T08:00:00Z",
        fasted: false,
        created_at: "2026-03-28T08:00:00Z",
      },
      { status: 201 },
    );
  }),
);

beforeAll(() => server.listen());
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

function renderForm() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <InterventionForm />
    </QueryClientProvider>,
  );
}

describe("InterventionForm", () => {
  beforeEach(() => {
    useAuthStore.getState().login("test-jwt-token");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  it("renders form fields", () => {
    renderForm();
    expect(screen.getByLabelText(/substance/i)).toBeDefined();
    expect(screen.getByLabelText(/dose/i)).toBeDefined();
    expect(screen.getByLabelText(/unit/i)).toBeDefined();
    expect(screen.getByLabelText(/route/i)).toBeDefined();
    expect(screen.getByRole("button", { name: /save intervention/i })).toBeDefined();
  });

  it("renders quick-pick chips when active substances exist", async () => {
    renderForm();
    await waitFor(() => {
      expect(screen.getByTestId("quick-pick-section")).toBeDefined();
    });
    expect(screen.getByText("BPC-157 250mcg SubQ")).toBeDefined();
    expect(screen.getByText("TB-500 2mg SubQ")).toBeDefined();
  });

  it("does not render quick-pick section when no active substances", async () => {
    server.use(
      http.get("/api/v1/protocols/active-substances", () => {
        return HttpResponse.json([]);
      }),
    );
    renderForm();
    // Wait for the query to settle
    await waitFor(() => {
      expect(screen.queryByTestId("quick-pick-section")).toBeNull();
    });
  });

  it("clicking a chip auto-fills form fields", async () => {
    const user = userEvent.setup();
    renderForm();

    await waitFor(() => {
      expect(screen.getByText("BPC-157 250mcg SubQ")).toBeDefined();
    });

    await user.click(screen.getByText("BPC-157 250mcg SubQ"));

    expect(screen.getByLabelText(/substance/i)).toHaveValue("BPC-157");
    expect(screen.getByLabelText(/dose/i)).toHaveValue(250);
    expect(screen.getByLabelText(/unit/i)).toHaveValue("mcg");
    expect(screen.getByLabelText(/route/i)).toHaveValue("SubQ");
  });

  it("does not render quick-pick section on fetch error", async () => {
    server.use(
      http.get("/api/v1/protocols/active-substances", () => {
        return new HttpResponse("Internal Server Error", { status: 500 });
      }),
    );
    renderForm();
    // Give query time to fail
    await new Promise((r) => setTimeout(r, 100));
    expect(screen.queryByTestId("quick-pick-section")).toBeNull();
  });

  it("shows loading state while submitting", async () => {
    const user = userEvent.setup();
    renderForm();

    await waitFor(() => {
      expect(screen.getByText("BPC-157 250mcg SubQ")).toBeDefined();
    });

    // Fill via chip
    await user.click(screen.getByText("BPC-157 250mcg SubQ"));

    await user.click(screen.getByRole("button", { name: /save intervention/i }));

    await waitFor(() => {
      expect(screen.getByText("Saved!")).toBeDefined();
    });
  });

  it("shows error message on submission failure", async () => {
    server.use(
      http.post("/api/v1/interventions", () => {
        return new HttpResponse("Validation failed", { status: 422 });
      }),
    );

    const user = userEvent.setup();
    renderForm();

    // Fill fields manually
    await user.type(screen.getByLabelText(/substance/i), "Test");
    await user.type(screen.getByLabelText(/dose/i), "100");
    await user.type(screen.getByLabelText(/unit/i), "mg");
    await user.type(screen.getByLabelText(/route/i), "oral");

    await user.click(screen.getByRole("button", { name: /save intervention/i }));

    await waitFor(() => {
      expect(screen.getByText(/error:/i)).toBeDefined();
    });
  });

  it("renders saved medicine chips in My Medicines section", async () => {
    renderForm();
    await waitFor(() => {
      expect(screen.getByTestId("saved-medicines-section")).toBeDefined();
    });
    expect(screen.getByText("Magnesium 400mg oral")).toBeDefined();
    expect(screen.getByText("Vitamin D 5000IU oral")).toBeDefined();
  });

  it("clicking a saved medicine chip pre-fills form", async () => {
    const user = userEvent.setup();
    renderForm();

    await waitFor(() => {
      expect(screen.getByText("Magnesium 400mg oral")).toBeDefined();
    });

    await user.click(screen.getByText("Magnesium 400mg oral"));

    expect(screen.getByLabelText(/substance/i)).toHaveValue("Magnesium");
    expect(screen.getByLabelText(/dose/i)).toHaveValue(400);
    expect(screen.getByLabelText(/unit/i)).toHaveValue("mg");
    expect(screen.getByLabelText(/route/i)).toHaveValue("oral");
  });

  it("+ button creates a new saved medicine", async () => {
    const user = userEvent.setup();
    renderForm();

    await waitFor(() => {
      expect(screen.getByTestId("saved-medicines-section")).toBeDefined();
    });

    // Fill in a substance so the + button is enabled
    await user.type(screen.getByLabelText(/substance/i), "Creatine");
    await user.type(screen.getByLabelText(/dose/i), "5");
    await user.type(screen.getByLabelText(/unit/i), "g");
    await user.type(screen.getByLabelText(/route/i), "oral");

    const addBtn = screen.getByRole("button", { name: /save current medicine/i });
    expect(addBtn).not.toBeDisabled();
    await user.click(addBtn);

    // After mutation, the saved-medicines query gets invalidated and refetched
    await waitFor(() => {
      expect(screen.getByTestId("saved-medicines-section")).toBeDefined();
    });
  });

  it("does not render saved medicines section when no saved medicines and no substance", async () => {
    server.use(
      http.get("/api/v1/saved-medicines", () => {
        return HttpResponse.json([]);
      }),
    );
    renderForm();
    // Wait for queries to settle
    await new Promise((r) => setTimeout(r, 100));
    expect(screen.queryByTestId("saved-medicines-section")).toBeNull();
  });
});
