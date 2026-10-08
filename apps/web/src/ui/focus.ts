import { useCallback, useEffect, useState } from "react";

/** Where focus should go; it is looked up after the commit, when the element exists. */
export type FocusTarget = () => HTMLElement | null | undefined;

/**
 * Moves focus to a target after the next commit (doc/design/accessibility.md, „Focus“).
 *
 * Call the returned function in the same event as the state change that removes the focused
 * element, for example together with the state that closes a dialog. The target is looked up when
 * React has committed that change, so no timer is needed. React Aria 1.x restores a dialog's focus
 * only if focus is on the body (current behavior, pinned by focus.test.tsx), so a move in the same commit wins.
 */
export function useFocusAfterCommit(): (target: FocusTarget) => void {
  // A new object for each request, so that a second request to the same target runs again.
  const [request, setRequest] = useState<{ target: FocusTarget }>();

  useEffect(() => {
    request?.target()?.focus();
  }, [request]);

  return useCallback((target: FocusTarget) => setRequest({ target }), []);
}

export interface Retry {
  /** True after a press on „Erneut versuchen“: the next failure of the area takes focus. */
  retried: boolean;
  /** Call it in the event of the press. `load` resolves to true when the area loaded. */
  retry: (load: () => Promise<boolean>) => void;
}

/**
 * The focus rule of „Erneut versuchen“ (doc/design/accessibility.md, „Focus“). The retry button
 * leaves with its InlineError. So a failure after a retry takes focus, and a success moves focus
 * to `target`, the heading of the area that loaded again.
 */
export function useRetry(target: FocusTarget): Retry {
  const [retried, setRetried] = useState(false);
  const focusAfterCommit = useFocusAfterCommit();
  const retry = useCallback(
    (load: () => Promise<boolean>) => {
      setRetried(true);
      void load().then((loaded) => {
        if (loaded) {
          setRetried(false);
          focusAfterCommit(target);
        }
      });
    },
    [focusAfterCommit, target],
  );
  return { retried, retry };
}

/** The first field of a form that shows an error, for the focus after a failed submit. */
export function firstInvalidField(form: HTMLElement | null): HTMLElement | null {
  return form?.querySelector<HTMLElement>("[aria-invalid='true']") ?? null;
}
