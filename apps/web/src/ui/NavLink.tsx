import type { Icon } from "@tabler/icons-react";
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
