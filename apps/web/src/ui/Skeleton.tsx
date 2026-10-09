import styles from "./Skeleton.module.css";

/** A gray block in the shape of a line of content. */
export function Skeleton() {
  return <span className={styles.skeleton} aria-hidden="true" />;
}

/** Three skeleton lines that stand in for content while it loads; `label` names what loads. */
export function SkeletonLines({ label }: { label: string }) {
  return (
    <div className={styles.lines} role="status" aria-label={label}>
      <Skeleton />
      <Skeleton />
      <Skeleton />
    </div>
  );
}
