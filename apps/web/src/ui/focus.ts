import { useCallback, useEffect, useState } from "react";

/** Where focus should go; it is looked up after the commit, when the element exists. */
export type FocusTarget = () => HTMLElement | null | undefined;

/**
 * Moves focus to a target after the next commit (doc/design/accessibility.md, „Focus“).
 *
 * Call the returned function in the same event as the state change that removes the focused
 * element, for example together with the state that closes a dialog. The target is looked up when
 * React has committed that change, so no timer is needed. A dialog from React Aria restores focus
 * to its trigger only if focus is on the body, so a focus move in the same commit wins.
 */
export function useFocusAfterCommit(): (target: FocusTarget) => void {
  // A new object for each request, so that a second request to the same target runs again.
  const [request, setRequest] = useState<{ target: FocusTarget }>();

  useEffect(() => {
    request?.target()?.focus();
  }, [request]);

  return useCallback((target: FocusTarget) => setRequest({ target }), []);
}

/** The first field of a form that shows an error, for the focus after a failed submit. */
export function firstInvalidField(form: HTMLElement | null): HTMLElement | null {
  return form?.querySelector<HTMLElement>("[aria-invalid='true']") ?? null;
}
