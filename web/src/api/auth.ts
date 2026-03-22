// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";
import { useAuthStore } from "../store/auth";

export interface TokenResponse {
  access_token: string;
  token_type: string;
  expires_in: number;
}

export async function login(
  email: string,
  password: string,
): Promise<void> {
  const data = await api.post<TokenResponse>("/api/v1/auth/login", {
    email,
    password,
  });
  useAuthStore.getState().login(data.access_token);
}

export async function refreshToken(): Promise<boolean> {
  try {
    const response = await fetch("/api/v1/auth/refresh", {
      method: "POST",
      credentials: "include",
    });
    if (!response.ok) return false;
    const data: TokenResponse = await response.json();
    useAuthStore.getState().login(data.access_token);
    return true;
  } catch {
    return false;
  }
}

export async function logout(): Promise<void> {
  try {
    const token = useAuthStore.getState().token;
    await fetch("/api/v1/auth/logout", {
      method: "POST",
      credentials: "include",
      headers: token ? { Authorization: `Bearer ${token}` } : {},
    });
  } finally {
    useAuthStore.getState().logout();
  }
}
