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
  username: string,
  password: string,
): Promise<void> {
  const data = await api.post<TokenResponse>("/api/v1/auth/login", {
    username,
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

export async function loginWithApple(idToken: string): Promise<void> {
  await api.post("/api/v1/auth/apple/callback", {
    id_token: idToken,
    platform: "web",
  });
  // Backend sets cookies; caller handles redirect/token refresh
}

export interface AuthMethod {
  id: string;
  provider: string;
  email: string | null;
  created_at: string;
}

export async function getAuthMethods(): Promise<AuthMethod[]> {
  return api.get<AuthMethod[]>("/api/v1/auth/methods");
}

export async function linkAuth(body: {
  provider: string;
  id_token?: string;
  password?: string;
}): Promise<AuthMethod[]> {
  return api.post<AuthMethod[]>("/api/v1/auth/link", body);
}

export async function unlinkAuth(provider: string): Promise<AuthMethod[]> {
  return api.delete<AuthMethod[]>(`/api/v1/auth/link/${provider}`);
}
