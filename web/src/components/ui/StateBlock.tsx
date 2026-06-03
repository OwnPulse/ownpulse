// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import type { ReactNode } from "react";

type StateBlockProps = {
  title?: string;
  children: ReactNode;
  tone?: "neutral" | "error";
};

export function StateBlock({ title, children, tone = "neutral" }: StateBlockProps) {
  const className = tone === "error" ? "op-state op-state-error" : "op-state";

  return (
    <div className={className} role={tone === "error" ? "alert" : undefined}>
      {title && <h2>{title}</h2>}
      <div>{children}</div>
    </div>
  );
}

export function LoadingState({ label = "Loading..." }: { label?: string }) {
  return <StateBlock>{label}</StateBlock>;
}

export function ErrorState({ message }: { message: string }) {
  return <StateBlock tone="error">{message}</StateBlock>;
}

export function EmptyState({ children }: { children: ReactNode }) {
  return <StateBlock>{children}</StateBlock>;
}
