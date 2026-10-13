import { type RefObject, useEffect, useState } from "react";

/** The width in rem from which the time axis replaces the list (a container query in the CSS). */
export const WIDE_REM = 40;

/**
 * True while the container is at least `WIDE_REM` wide. The CSS decides what the member sees; this
 * decides what the keyboard reaches, so that the list and the axis never both take the focus for
 * the same item. Without a `ResizeObserver` the container counts as narrow.
 */
export function useWide(ref: RefObject<HTMLElement | null>): boolean {
  const [wide, setWide] = useState(false);
  useEffect(() => {
    const element = ref.current;
    if (!element || typeof ResizeObserver === "undefined") {
      return;
    }
    const rem = Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
    const observer = new ResizeObserver(([entry]) => {
      setWide((entry?.contentRect.width ?? 0) >= WIDE_REM * rem);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [ref]);
  return wide;
}
