import type { ReactNode } from "react";
import { Link, type LinkProps } from "../router/Router";
import styles from "./NavLink.module.css";

export interface NavLinkProps extends Omit<LinkProps, "className"> {
  /** A large target, for the bottom bar on narrow layouts. */
  large?: boolean;
}

/**
 * A navigation item. The current page has `aria-current="page"` and the current style
 * (doc/design/layout-and-responsiveness.md, „Navigation“).
 */
export function NavLink({ large, ...props }: NavLinkProps) {
  return <Link {...props} className={large ? `${styles.link} ${styles.large}` : styles.link} />;
}

/** The sub-navigation of a page template, for example of the settings or of an event. */
export function SubNav({ label, children }: { label: string; children: ReactNode }) {
  return (
    <nav className={styles.subNav} aria-label={label}>
      {children}
    </nav>
  );
}
