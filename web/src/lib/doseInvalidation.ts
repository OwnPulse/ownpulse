// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import type { QueryClient } from "@tanstack/react-query";

// Every dose log/skip/undo mutation (DoseStatusGrid, TodaysDoses, and
// InterventionForm's protocol-attribution path) touches this same set of
// cached data — log/undo also create or delete an `interventions` row, so
// that list needs invalidating too, not just the protocol/dose views.
// `["run-adherence"]`/`["run-doses"]` invalidate every cached run, not just
// the mutated one — TanStack Query's default `invalidateQueries` matching
// is prefix-based, so this covers the currently-open run without the caller
// having to know its id.
const DOSE_MUTATION_QUERY_KEYS: readonly (readonly string[])[] = [
  ["todays-doses"],
  ["missed-doses"],
  ["active-runs"],
  ["protocols"],
  ["interventions"],
  ["run-adherence"],
  ["run-doses"],
];

export function invalidateDoseQueries(queryClient: QueryClient) {
  for (const queryKey of DOSE_MUTATION_QUERY_KEYS) {
    queryClient.invalidateQueries({ queryKey: [...queryKey] });
  }
}
