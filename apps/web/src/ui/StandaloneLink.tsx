import { Link, type LinkProps } from "../router/Router";
import styles from "./StandaloneLink.module.css";

/**
 * A link that is no part of a sentence, for example in a table cell. It has the pointer target
 * size of the density. A link inside a sentence is exempt (WCAG 2.5.8) and uses `Link`.
 */
export function StandaloneLink({ className, ...props }: LinkProps) {
  return (
    <Link
      {...props}
      className={className ? `${styles.standalone} ${className}` : styles.standalone}
    />
  );
}
