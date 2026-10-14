import type { Icon } from "@tabler/icons-react";
import type { ReactNode } from "react";
import { Link, type LinkProps } from "../router/Router";
import styles from "./NavLink.module.css";

export interface NavLinkProps extends Omit<LinkProps, "className"> {
  /** A large target, for the bottom bar on narrow layouts. */
  large?: boolean;
  /** A drawn icon before the label. It is decoration: the label names the item. */
  icon?: Icon;
}

/**
 * A navigation item. The current page has `aria-current="page"` and the current style
 * (doc/design/layout-and-responsiveness.md, „Navigation“).
 */
export function NavLink({ large, icon: Icon, children, ...props }: NavLinkProps) {
  return (
    <Link {...props} className={large ? `${styles.link} ${styles.large}` : styles.link}>
      {Icon && <Icon size={16} stroke={1.5} aria-hidden="true" className={styles.icon} />}
      {children}
    </Link>
  );
}

/**
 * The sub-navigation of a page template, for example of the settings or of an event. The
 * `secondary` level is a second row below an item of the first, for example the registers.
 */
export function SubNav({
  label,
  level = "primary",
  children,
}: {
  label: string;
  level?: "primary" | "secondary";
  children: ReactNode;
}) {
  return (
    <nav className={styles.subNav} data-level={level} aria-label={label}>
      {children}
    </nav>
  );
}
