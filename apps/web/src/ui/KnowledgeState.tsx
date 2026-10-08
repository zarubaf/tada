import type { ReactNode } from "react";
import { t } from "../i18n";
import styles from "./KnowledgeState.module.css";

export type KnowledgeStateName = "accepted" | "proposed" | "assumption" | "unknown";

export interface KnowledgeStateProps {
  state: KnowledgeStateName;
  /** The value. An unknown value has none: the label is the text. */
  children?: ReactNode;
  /**
   * Shows the label of an accepted value. The other states always show it. Detail views and the
   * evidence panel set it (doc/design/tokens.md, „State of knowledge“).
   */
  showLabel?: boolean;
}

/** The mark of each state: a circle that differs in line style, with a check or a question mark. */
const MARKS: Record<KnowledgeStateName, ReactNode> = {
  accepted: <path d="M5.5 8.5 7.2 10.2 10.7 6.2" />,
  proposed: null,
  assumption: null,
  unknown: <path d="M6.5 6.5a1.6 1.6 0 1 1 2.3 1.4c-.5.3-.8.6-.8 1.2M8 11.4v.1" />,
};

function Icon({ state }: { state: KnowledgeStateName }) {
  return (
    <svg
      className={styles.icon}
      viewBox="0 0 16 16"
      width="16"
      height="16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      <circle
        cx="8"
        cy="8"
        r="6.25"
        strokeDasharray={
          state === "proposed" ? "3 2.2" : state === "assumption" ? "0.1 2.6" : undefined
        }
      />
      {MARKS[state]}
    </svg>
  );
}

/**
 * The state of knowledge of a value: its text, an icon and a label (doc/design/components.md).
 * Color and line style only support the label. An unknown value shows „Unbekannt“, never a gap.
 */
export function KnowledgeState({ state, children, showLabel = false }: KnowledgeStateProps) {
  const label = t(`knowledge-${state}`);
  const labelVisible = showLabel || state !== "accepted";
  return (
    <span className={styles.state} data-state={state} title={label}>
      {children !== undefined && <span className={styles.value}>{children}</span>}
      <span className={styles.mark}>
        <Icon state={state} />
        <span className={labelVisible ? styles.label : styles.hidden}>{label}</span>
      </span>
    </span>
  );
}
