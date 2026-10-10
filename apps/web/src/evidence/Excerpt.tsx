import type { DisputeNote } from "../api/client";
import { DisputeNotes } from "../ui/DisputeNotes";
import { Redacted } from "../ui/Redacted";
import styles from "./Excerpt.module.css";

export interface ExcerptProps {
  /** The text before the passage, if the source gives it. */
  before?: string;
  /** The cited passage. It is marked. */
  quote: string;
  /** The text after the passage, if the source gives it. */
  after?: string;
  /** True after a legal redaction of the source: „entfernt“ replaces the texts (ADR 0072). */
  redacted?: boolean;
  /** The dispute notes about the source of the passage (ADR 0073). */
  notes?: DisputeNote[] | undefined;
}

/** A source excerpt with the cited passage marked (doc/design/components.md, „Evidence panel“). */
export function Excerpt({ before, quote, after, redacted, notes }: ExcerptProps) {
  return (
    <>
      {redacted ? (
        <p>
          <Redacted />
        </p>
      ) : (
        <blockquote className={styles.quote}>
          {before}
          <mark>{quote}</mark>
          {after}
        </blockquote>
      )}
      <DisputeNotes notes={notes} />
    </>
  );
}
