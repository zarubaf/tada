/**
 * Reads `#token=` of the address and removes the fragment from the address bar (ADR 0056), so that
 * the token does not stay in the history, the screen or a copied link. Call it once per page.
 */
export function takeFragmentToken(): string | undefined {
  const token = new URLSearchParams(window.location.hash.slice(1)).get("token");
  if (window.location.hash) {
    const { pathname, search } = window.location;
    window.history.replaceState(null, "", `${pathname}${search}`);
  }
  return token || undefined;
}
