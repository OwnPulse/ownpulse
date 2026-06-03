// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import TelemetrySettings from "../../src/components/settings/TelemetrySettings";
import { isTelemetryEnabled, setTelemetryEnabled } from "../../src/lib/telemetry";

beforeEach(() => {
  localStorage.clear();
});

afterEach(() => {
  setTelemetryEnabled(false);
  localStorage.clear();
});

describe("TelemetrySettings", () => {
  it("renders the opt-in block with telemetry OFF by default", () => {
    render(<TelemetrySettings />);
    expect(screen.getByRole("heading", { name: /anonymous usage telemetry/i })).toBeInTheDocument();
    const checkbox = screen.getByRole("checkbox");
    expect(checkbox).not.toBeChecked();
    expect(screen.getByText(/telemetry disabled/i)).toBeInTheDocument();
  });

  it("reflects an already-enabled preference on mount", () => {
    setTelemetryEnabled(true);
    render(<TelemetrySettings />);
    expect(screen.getByRole("checkbox")).toBeChecked();
    expect(screen.getByText(/telemetry enabled/i)).toBeInTheDocument();
  });

  it("enables telemetry when toggled on", async () => {
    const user = userEvent.setup();
    render(<TelemetrySettings />);
    const checkbox = screen.getByRole("checkbox");

    await user.click(checkbox);

    expect(checkbox).toBeChecked();
    expect(isTelemetryEnabled()).toBe(true);
    expect(screen.getByText(/telemetry enabled/i)).toBeInTheDocument();
  });

  it("disables telemetry when toggled off again", async () => {
    const user = userEvent.setup();
    setTelemetryEnabled(true);
    render(<TelemetrySettings />);
    const checkbox = screen.getByRole("checkbox");
    expect(checkbox).toBeChecked();

    await user.click(checkbox);

    expect(checkbox).not.toBeChecked();
    expect(isTelemetryEnabled()).toBe(false);
  });

  it("makes the privacy guarantees visible to the user", () => {
    render(<TelemetrySettings />);
    expect(screen.getByText(/never any health data/i)).toBeInTheDocument();
    expect(screen.getByText(/never to third parties/i)).toBeInTheDocument();
  });
});
