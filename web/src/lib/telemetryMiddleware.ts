// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

// Zustand middleware that emits a first-party `action` telemetry event whenever
// a store mutation is performed with an explicit action label. The label is the
// optional third argument to `set` (the same slot the devtools middleware uses),
// so only deliberately-named mutations are tracked — anonymous `set(...)` calls
// emit nothing. The label is a coarse, developer-chosen string and never carries
// user content. Tracking is gated by the opt-in check inside `trackAction`.

import type { StateCreator, StoreMutatorIdentifier } from "zustand";
import { trackAction } from "./telemetry";

// Mutator that widens the store's `set`/`setState` to accept an optional action
// label as a third argument, so callers can name a mutation for telemetry.
type WithTelemetry<S> = Write<S, StoreSetWithLabel<S>>;
type Write<T, U> = Omit<T, keyof U> & U;
type StoreSetWithLabel<S> = S extends { setState: infer SetState }
  ? {
      setState: SetState extends (partial: infer P, replace?: infer R, ...a: never[]) => infer Ret
        ? (partial: P, replace?: R, label?: string) => Ret
        : SetState;
    }
  : never;

declare module "zustand" {
  // The `A` type parameter is part of Zustand's StoreMutators signature; this
  // mutator doesn't use it but must keep the arity to merge correctly.
  interface StoreMutators<S, A> {
    "ownpulse/telemetry": WithTelemetry<S> & Record<never, A>;
  }
}

type Telemetry = <
  T,
  Mps extends [StoreMutatorIdentifier, unknown][] = [],
  Mcs extends [StoreMutatorIdentifier, unknown][] = [],
>(
  initializer: StateCreator<T, [...Mps, ["ownpulse/telemetry", never]], Mcs>,
) => StateCreator<T, Mps, [["ownpulse/telemetry", never], ...Mcs]>;

type TelemetryImpl = <T>(initializer: StateCreator<T, [], []>) => StateCreator<T, [], []>;

const telemetryImpl: TelemetryImpl = (initializer) => (set, get, store) => {
  const trackedSet: typeof set = (...args) => {
    // Zustand passes an optional action label as the argument after `replace`.
    const label = (args as unknown[])[2] as string | undefined;
    (set as (...a: unknown[]) => void)(...(args as unknown[]));
    if (typeof label === "string" && label !== "") {
      trackAction(label);
    }
  };
  store.setState = trackedSet as typeof store.setState;
  return initializer(trackedSet, get, store);
};

export const telemetry = telemetryImpl as unknown as Telemetry;
