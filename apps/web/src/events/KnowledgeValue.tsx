import type { ReactNode } from "react";
import { t } from "../i18n";
import { Chip } from "../ui/Chip";
import { KNOWLEDGE_ICONS, type KnowledgeStateName } from "../ui/KnowledgeState";
import styles from "./KnowledgeValue.module.css";

/**
 * A value and the state of knowledge that it has, as a chip beside it (ADR 0082): „Vorschlag“,
 * „Annahme“ or „Unbekannt“. A confirmed value has no chip, but its word stays for assistive
 * technology. An unknown value has no text of its own.
 */
export function KnowledgeValue({
  state,
  children,
}: {
  state: KnowledgeStateName;
  children?: ReactNode;
}) {
  return (
    <span className={styles.value}>
      {children !== undefined && <span className={styles.text}>{children}</span>}
      {state === "accepted" && <span className={styles.hidden}>{t("knowledge-accepted")}</span>}
      {state === "proposed" && <Chip variant="proposal" />}
      {state === "assumption" && <Chip variant="assumption" />}
      {state === "unknown" && <Chip variant="unknown" />}
      {state === "conflict" && (
        <Chip icon={KNOWLEDGE_ICONS.conflict}>{t("knowledge-conflict")}</Chip>
      )}
    </span>
  );
}
