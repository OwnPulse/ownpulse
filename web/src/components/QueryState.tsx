// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import type { ReactNode } from "react";

interface QueryStateProps {
  isLoading: boolean;
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
 */
export function QueryState({
  isLoading,
  isError,
  onRetry,
  loadingText = "Loading...",
  errorText = "Something went wrong.",
  children,
}: QueryStateProps) {
  if (isLoading) {
    return <p className="op-loading">{loadingText}</p>;
  }

  if (isError) {
    return (
      <div className="op-error-state">
        <p className="op-error-msg">{errorText}</p>
        {onRetry && (
          <button type="button" className="op-btn op-btn-secondary op-btn-sm" onClick={onRetry}>
            Retry
          </button>
        )}
      </div>
    );
  }

  return <>{children}</>;
}
