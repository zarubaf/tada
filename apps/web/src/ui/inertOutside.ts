/** Live regions and the toast region stay readable: they announce while a sheet is open. */
const KEEP =
  '[aria-live], [role="status"], [role="alert"], [data-live-announcer], [data-toast-region]';

/**
 * Makes everything outside `keep` inert: the siblings of `keep` and of each of its ancestors up to
 * `document.body`. The ancestors themselves stay live.
 *
 * - A keep element (live region, toast region) stays live. A sibling that contains one is not made
 *   inert itself: its children are visited instead, so a live region inside a header keeps announcing.
 * - A node that appears later is treated the same way, wherever it appears in a visited container.
 * - A node that appears later directly in `document.body` is a portal root (a popover, a menu, a
 *   dialog or a toast opened from the panel). It stays live.
 *
 * Returns a function that restores every element it changed. Elements that were inert before are
 * left as they were.
 */
export function makeOutsideInert(keep: HTMLElement): () => void {
  const changed = new Set<HTMLElement>();
  const observers = new Map<Element, MutationObserver>();
  const known = new WeakSet<Element>();

  function visit(element: HTMLElement) {
    if (element.inert || element.matches(KEEP)) {
      return;
    }
    if (element.querySelector(KEEP)) {
      watch(element, null);
      return;
    }
    element.inert = true;
    changed.add(element);
  }

  /** Visits the children of `container` except `path`; watches the container for new children. */
  function watch(container: Element, path: Element | null) {
    const sweep = () => {
      for (const child of container.children) {
        const isNew = !known.has(child);
        known.add(child);
        if (!(child instanceof HTMLElement) || child === path) {
          continue;
        }
        if (isNew && sweepDone && container === document.body) {
          continue;
        }
        visit(child);
      }
    };
    if (!observers.has(container)) {
      const observer = new MutationObserver(sweep);
      observer.observe(container, { childList: true });
      observers.set(container, observer);
    }
    sweep();
  }

  let sweepDone = false;
  let path: Element = keep;
  let parent = keep.parentElement;
  while (parent && parent !== document.documentElement) {
    watch(parent, path);
    path = parent;
    parent = parent.parentElement;
  }
  sweepDone = true;
  document.documentElement.style.overflow = "hidden";

  return () => {
    for (const observer of observers.values()) {
      observer.disconnect();
    }
    for (const element of changed) {
      element.inert = false;
    }
    document.documentElement.style.overflow = "";
  };
}
