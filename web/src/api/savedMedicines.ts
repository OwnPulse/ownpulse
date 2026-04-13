// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";

export interface SavedMedicine {
  id: string;
  substance: string;
  dose: number | null;
  unit: string | null;
  route: string | null;
  sort_order: number;
  created_at: string;
}

export interface CreateSavedMedicine {
  substance: string;
  dose?: number;
  unit?: string;
  route?: string;
}

export const savedMedicinesApi = {
  list: () => api.get<SavedMedicine[]>("/api/v1/saved-medicines"),
  create: (data: CreateSavedMedicine) => api.post<SavedMedicine>("/api/v1/saved-medicines", data),
  update: (id: string, data: Partial<CreateSavedMedicine>) =>
    api.put<SavedMedicine>(`/api/v1/saved-medicines/${id}`, data),
  remove: (id: string) => api.delete<void>(`/api/v1/saved-medicines/${id}`),
};
