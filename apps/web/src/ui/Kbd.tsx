import styles from "./Kbd.module.css";

/** A keyboard key in a hint, for example `Ctrl K`. The text states the key, so it is not hidden. */
export function Kbd({ children }: { children: string }) {
  return <kbd className={styles.kbd}>{children}</kbd>;
}
