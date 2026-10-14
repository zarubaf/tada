import { useEffect, useState } from "react";

/**
 * The `#token=` of the address, kept in state. The render only reads it. An effect removes the
 * fragment from the address bar (ADR 0056), so that the token does not stay in the history, the
 * screen or a copied link. A render that React throws away (a chunk or a state update in between)
 * runs again and finds the token still there; removing it in the render would lose it.
 */
export function useFragmentToken(): string | undefined {
  const [token] = useState(
    () => new URLSearchParams(window.location.hash.slice(1)).get("token") || undefined,
  );
  useEffect(() => {
    if (window.location.hash) {
      const { pathname, search } = window.location;
      window.history.replaceState(null, "", `${pathname}${search}`);
    }
  }, []);
  return token;
}
