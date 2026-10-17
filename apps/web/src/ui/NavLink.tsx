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

export interface DisabledNavItemProps {
  icon?: Icon;
  /** The ID of the element that says why the item is off: a screen reader reads it with the label. */
  describedBy: string;
  children: ReactNode;
}

/**
 * A navigation item that has no target yet. It stays in the list so that the menu does not change,
 * and it keeps keyboard focus so that a screen reader reads the reason. It has no `href`, so a
 * press goes nowhere.
 */
export function DisabledNavItem({ icon: Icon, describedBy, children }: DisabledNavItemProps) {
  return (
    // biome-ignore lint/a11y/useSemanticElements: a link without a target is not an `a` element
    <span
      role="link"
      aria-disabled="true"
      aria-describedby={describedBy}
      tabIndex={0}
      className={`${styles.link} ${styles.disabled}`}
    >
      {Icon && <Icon size={16} stroke={1.5} aria-hidden="true" className={styles.icon} />}
      {children}
    </span>
  );
}
