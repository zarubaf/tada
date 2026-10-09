import type { Ref } from "react";
import styles from "./LiveRegion.module.css";

export interface LiveRegionProps {
  /** `status` is polite: a result. `alert` is assertive: an error that blocks the task. */
  kind: "status" | "alert";
  /** The text. It is empty until there is a message. */
  children?: string | undefined;
  /** Hides the region from the eye, when another element shows the same text. */
  visuallyHidden?: boolean;
  /** For the place of the region in the layout of the page. */
  className?: string;
  ref?: Ref<HTMLParagraphElement>;
}

/**
 * A live region that is in the page before its text. Screen readers announce a text that changes
 * inside a region that exists; a region that appears with its text is often not announced
 * (doc/design/accessibility.md, „Status and live regions“). An empty region takes no space.
 */
export function LiveRegion({ kind, children, visuallyHidden, className, ref }: LiveRegionProps) {
  const classes = [
    styles.region,
    visuallyHidden ? styles.hidden : kind === "alert" && styles.alert,
    className,
  ];
  return (
    <p ref={ref} className={classes.filter(Boolean).join(" ")} role={kind}>
      {children}
    </p>
  );
}
