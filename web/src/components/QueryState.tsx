// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import type { ReactNode } from "react";

interface QueryStateProps {
  isLoading: boolean;
  /**
   * Pass the query's `isFetching` alongside `isError` so a Retry click gives
   * feedback: in TanStack Query v5, `isLoading` is `isPending && isFetching`,
   * which is never true again once a query has errored once — so without
   * this, clicking Retry after an error leaves the error view frozen (and
   * re-clickable) while the refetch is in flight.
   */
  isFetching?: boolean;
  isError?: boolean;
  onRetry?: () => void;
  loadingText?: string;
  errorText?: string;
  children: ReactNode;
}

/**
 * Shared loading/error rendering for TanStack Query-backed views. Keeps the
 * bare-text loading/error states used across pages consistent and gives every
 * error state a Retry action wired to the query's refetch.
 *
 * The status container is always mounted (only its text content is
 * conditional) so assistive tech reliably announces loading/error/retry
 * transitions instead of relying on a role="status" node being inserted
 * fresh into the DOM each time.
 */
export function QueryState({
  isLoading,
  isFetching,
  isError,
  onRetry,
  loadingText = "Loading...",
  errorText = "Something went wrong.",
  children,
}: QueryStateProps) {
  const retrying = !!isError && !!isFetching;

  return (
    <>
      <div
        className={isLoading ? "op-loading" : isError ? "op-error-state" : undefined}
        role="status"
        aria-live="polite"
      >
        {isLoading && loadingText}
        {isError && (
          <>
            <p className="op-error-msg">{errorText}</p>
            {onRetry && (
              <button
                type="button"
                className="op-btn op-btn-secondary op-btn-sm"
                onClick={() => onRetry()}
                disabled={retrying}
              >
                {retrying ? "Retrying…" : "Retry"}
              </button>
            )}
          </>
        )}
      </div>
      {!isLoading && !isError && children}
    </>
  );
}
