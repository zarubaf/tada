import { Link, type LinkProps } from "../router/Router";
import styles from "./StandaloneLink.module.css";

/**
 * A link that is no part of a sentence, for example in a table cell. It has the pointer target
 * size of the density. A link inside a sentence is exempt (WCAG 2.5.8) and uses `Link`.
 */
export function StandaloneLink({
  className,
  inRow,
  ...props
}: LinkProps & {
  /** The link stands in a table row, which must not grow taller than `--row-height`. */
  inRow?: boolean;
}) {
  const classes = [styles.standalone, inRow && styles.inRow, className].filter(Boolean);
  return <Link {...props} className={classes.join(" ")} />;
}
