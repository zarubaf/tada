import styles from "./Skeleton.module.css";

/** A gray block in the shape of a line of content. */
export function Skeleton() {
  return <span className={styles.skeleton} aria-hidden="true" />;
}
