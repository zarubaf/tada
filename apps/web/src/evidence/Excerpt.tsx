import styles from "./Excerpt.module.css";

export interface ExcerptProps {
  /** The text before the passage, if the source gives it. */
  before?: string;
  /** The cited passage. It is marked. */
  quote: string;
  /** The text after the passage, if the source gives it. */
  after?: string;
}

/** A source excerpt with the cited passage marked (doc/design/components.md, „Evidence panel“). */
export function Excerpt({ before, quote, after }: ExcerptProps) {
  return (
    <blockquote className={styles.quote}>
      {before}
      <mark>{quote}</mark>
      {after}
    </blockquote>
  );
}
