// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { QueryState } from "../../src/components/QueryState";

describe("QueryState", () => {
  it("renders children when not loading and not errored", () => {
    render(
      <QueryState isLoading={false} isError={false}>
        <p>Content</p>
      </QueryState>,
    );
    expect(screen.getByText("Content")).toBeDefined();
  });

  it("renders loading text while loading", () => {
    render(
      <QueryState isLoading={true} loadingText="Loading widgets...">
        <p>Content</p>
      </QueryState>,
    );
    expect(screen.getByText("Loading widgets...")).toBeDefined();
    expect(screen.queryByText("Content")).toBeNull();
  });

  it("renders error text and does not render children on error", () => {
    render(
      <QueryState isLoading={false} isError={true} errorText="Could not load widgets.">
        <p>Content</p>
      </QueryState>,
    );
    expect(screen.getByText("Could not load widgets.")).toBeDefined();
    expect(screen.queryByText("Content")).toBeNull();
  });

  it("calls onRetry when the Retry button is clicked", async () => {
    const onRetry = vi.fn();
    const user = userEvent.setup();
    render(
      <QueryState isLoading={false} isError={true} onRetry={onRetry}>
        <p>Content</p>
      </QueryState>,
    );
    await user.click(screen.getByRole("button", { name: /retry/i }));
    expect(onRetry).toHaveBeenCalledOnce();
  });

  it("does not render a Retry button when onRetry is not provided", () => {
    render(
      <QueryState isLoading={false} isError={true}>
        <p>Content</p>
      </QueryState>,
    );
    expect(screen.queryByRole("button", { name: /retry/i })).toBeNull();
  });
});
