// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useAuthStore } from "../store/auth";

async function downloadExport(format: "json" | "csv"): Promise<void> {
  const token = useAuthStore.getState().token;
  const response = await fetch(`/api/v1/export/${format}`, {
    method: "GET",
    credentials: "include",
    headers: token ? { Authorization: `Bearer ${token}` } : {},
  });

  if (!response.ok) {
    throw new Error(`Export failed: ${response.statusText}`);
  }

  const blob = await response.blob();
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `ownpulse-export.${format}`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

export function exportJson(): Promise<void> {
  return downloadExport("json");
}

export function exportCsv(): Promise<void> {
  return downloadExport("csv");
}
