/** Live regions and the toast region stay readable: they announce while a sheet is open. */
const KEEP =
  '[aria-live], [role="status"], [role="alert"], [data-live-announcer], [data-toast-region]';

/**
 * Makes everything outside `keep` inert: the siblings of `keep` and of each of its ancestors up to
 * `document.body`. The ancestors themselves stay live. Live regions stay live. A sibling that
 * appears later is made inert too. Returns a function that restores every element it changed.
 * Elements that were inert before are left as they were.
 */
export function makeOutsideInert(keep: HTMLElement): () => void {
  const changed = new Set<HTMLElement>();
  const observers: MutationObserver[] = [];

  function mark(parent: HTMLElement, path: Element) {
    for (const child of parent.children) {
      if (child instanceof HTMLElement && child !== path && !child.inert && !child.matches(KEEP)) {
        child.inert = true;
        changed.add(child);
      }
    }
  }

  let path: Element = keep;
  let parent = keep.parentElement;
  while (parent && parent !== document.documentElement) {
    const owner = parent;
    const current = path;
    mark(owner, current);
    const observer = new MutationObserver(() => mark(owner, current));
    observer.observe(owner, { childList: true });
    observers.push(observer);
    path = parent;
    parent = parent.parentElement;
  }
  document.documentElement.style.overflow = "hidden";

  return () => {
    for (const observer of observers) {
      observer.disconnect();
    }
    for (const element of changed) {
      element.inert = false;
    }
    document.documentElement.style.overflow = "";
  };
}
