// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useQuery } from "@tanstack/react-query";

export interface AppConfig {
  feature_flags: Record<string, boolean>;
  ios: {
    min_supported_version: string | null;
    force_upgrade_below: string | null;
  };
}

export function useAppConfig() {
  return useQuery<AppConfig>({
    queryKey: ["app-config"],
    queryFn: async () => {
      const res = await fetch("/api/v1/config");
      if (!res.ok) throw new Error("Failed to fetch app config");
      return res.json();
    },
    staleTime: 60_000,
    gcTime: 5 * 60_000,
    refetchOnWindowFocus: true,
  });
}

export function useFeatureFlag(key: string): boolean {
  const { data } = useAppConfig();
  return data?.feature_flags[key] ?? false;
}
