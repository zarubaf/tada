import { useSyncExternalStore } from "react";

/** The border to the narrow layout of ADR 0023, as in the style sheets (`--bp-medium`). */
const NARROW = "(width < 40rem)";

function subscribe(onChange: () => void): () => void {
  const query = window.matchMedia?.(NARROW);
  query?.addEventListener("change", onChange);
  return () => query?.removeEventListener("change", onChange);
}

/** True below 40rem. Without `matchMedia` the layout is not narrow. */
export function useNarrow(): boolean {
  return useSyncExternalStore(
    subscribe,
    () => window.matchMedia?.(NARROW).matches ?? false,
    () => false,
  );
}
