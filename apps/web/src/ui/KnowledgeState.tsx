import {
  IconCircleCheck,
  IconCircleDashed,
  IconCircleDotted,
  IconHelpCircle,
} from "@tabler/icons-react";
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

const ICONS = {
  accepted: IconCircleCheck,
  proposed: IconCircleDashed,
  assumption: IconCircleDotted,
  unknown: IconHelpCircle,
} satisfies Record<KnowledgeStateName, unknown>;

/**
 * The state of knowledge of a value: its text, an icon and a label (doc/design/components.md).
 * Color and line style only support the label. An unknown value shows „Unbekannt“, never a gap.
 */
export function KnowledgeState({ state, children, showLabel = false }: KnowledgeStateProps) {
  const label = t(`knowledge-${state}`);
  const Icon = ICONS[state];
  const labelVisible = showLabel || state !== "accepted";
  return (
    <span className={styles.state} data-state={state} title={label}>
      {children !== undefined && <span className={styles.value}>{children}</span>}
      <span className={styles.mark}>
        <Icon className={styles.icon} size={16} stroke={1.5} aria-hidden="true" />
        <span className={labelVisible ? styles.label : styles.hidden}>{label}</span>
      </span>
    </span>
  );
}
