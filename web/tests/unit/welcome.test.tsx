// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import Welcome from "../../src/pages/Welcome";

function renderWelcome() {
  return render(
    <MemoryRouter>
      <Welcome />
    </MemoryRouter>,
  );
}

describe("Welcome", () => {
  it("renders welcome title", () => {
    renderWelcome();
    expect(screen.getByText("Welcome to OwnPulse")).toBeInTheDocument();
  });

  it("renders all feature items", () => {
    renderWelcome();
    expect(screen.getByText("Track health metrics")).toBeInTheDocument();
    expect(screen.getByText("Log check-ins and interventions")).toBeInTheDocument();
    expect(screen.getByText("Upload genetic data")).toBeInTheDocument();
    expect(screen.getByText("Explore trends and correlations")).toBeInTheDocument();
  });

  it("renders getting started steps", () => {
    renderWelcome();
    expect(screen.getByText(/Connect a wearable data source/)).toBeInTheDocument();
    expect(screen.getByText(/Log your first daily check-in/)).toBeInTheDocument();
  });

  it("renders Go to Dashboard link that navigates to /", () => {
    renderWelcome();
    const link = screen.getByRole("link", { name: "Go to Dashboard" });
    expect(link).toHaveAttribute("href", "/");
  });
});
