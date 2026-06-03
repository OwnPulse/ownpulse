// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import type { ReactNode } from "react";

type PageProps = {
  title: string;
  actions?: ReactNode;
  children: ReactNode;
};

export function Page({ title, actions, children }: PageProps) {
  return (
    <main className="op-page">
      <div className="op-page-header">
        <h1>{title}</h1>
        {actions && <div className="op-page-actions">{actions}</div>}
      </div>
      {children}
    </main>
  );
}

type PageSectionProps = {
  title?: string;
  children: ReactNode;
};

export function PageSection({ title, children }: PageSectionProps) {
  return (
    <section className="op-section">
      {title && <h2 className="op-section-title">{title}</h2>}
      {children}
    </section>
  );
}
