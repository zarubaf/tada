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
  /** A large target, as of `NavLink`. */
  large?: boolean;
  /**
   * The ID of the element that says why the item is off: a screen reader reads it with the label.
   * Without it the item is a placeholder for a target that is on its way: it looks like a link,
   * is hidden from assistive technology and takes no focus, so the menu keeps its shape.
   */
  describedBy?: string;
  children: ReactNode;
}

/**
 * A navigation item that has no target. It stays in the list so that the menu does not change,
 * and a disabled one keeps keyboard focus so that a screen reader reads the reason. It has no
 * `href`, so a press goes nowhere.
 */
export function DisabledNavItem({
  icon: Icon,
  large,
  describedBy,
  children,
}: DisabledNavItemProps) {
  const size = large ? ` ${styles.large}` : "";
  const content = (
    <>
      {Icon && <Icon size={16} stroke={1.5} aria-hidden="true" className={styles.icon} />}
      {children}
    </>
  );
  if (describedBy === undefined) {
    return (
      <span aria-hidden="true" className={`${styles.link}${size}`}>
        {content}
      </span>
    );
  }
  return (
    // biome-ignore lint/a11y/useSemanticElements: a link without a target is not an `a` element
    <span
      role="link"
      aria-disabled="true"
      aria-describedby={describedBy}
      tabIndex={0}
      className={`${styles.link} ${styles.disabled}${size}`}
    >
      {content}
    </span>
  );
}
