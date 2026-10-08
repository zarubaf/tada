import type { Ref } from "react";

export interface LiveRegionProps {
  /** `status` is polite: a result. `alert` is assertive: an error that blocks the task. */
  kind: "status" | "alert";
  /** The text. It is empty until there is a message. */
  children?: string | undefined;
  className?: string;
  ref?: Ref<HTMLParagraphElement>;
}

/**
 * A live region that is in the page before its text. Screen readers announce a text that changes
 * inside a region that exists; a region that appears with its text is often not announced
 * (doc/design/accessibility.md, „Status and live regions“).
 */
export function LiveRegion({ kind, children, className, ref }: LiveRegionProps) {
  return (
    <p ref={ref} className={className} role={kind}>
      {children}
    </p>
  );
}
