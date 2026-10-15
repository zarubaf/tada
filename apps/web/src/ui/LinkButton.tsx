import type { ReactNode } from "react";
import { Link } from "../router/Router";
import buttonStyles from "./Button.module.css";
import styles from "./LinkButton.module.css";

/**
 * A link that looks like a button, for an action that opens a page, for example a form. It is a
 * link for the keyboard and for assistive technology (doc/design/components.md).
 */
export function LinkButton({
  to,
  primary,
  compact,
  children,
}: {
  to: string;
  primary?: boolean;
  /** The compact size of a toolbar row, like `size="sm"` of `Button`. */
  compact?: boolean;
  children: ReactNode;
}) {
  return (
    <Link
      to={to}
      className={[
        buttonStyles.button,
        styles.link,
        primary && styles.primary,
        compact && styles.compact,
      ]
        .filter(Boolean)
        .join(" ")}
    >
      {children}
    </Link>
  );
}
