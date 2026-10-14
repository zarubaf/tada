import { type Icon, IconCalendar, IconCalendarExclamation, IconStack2 } from "@tabler/icons-react";
import type { ReactNode } from "react";
import { Button as AriaButton } from "react-aria-components";
import { t } from "../i18n";
import styles from "./Chip.module.css";
import { KNOWLEDGE_ICONS, type KnowledgeStateName } from "./KnowledgeState";

export type ChipVariant = "neutral" | "date" | "workstream" | "proposal" | "assumption" | "unknown";

export interface ChipProps {
  variant?: ChipVariant;
  /**
   * The text. For a proposal, an assumption or an unknown value it is the detail after the label,
   * for example the proposed date, and may be empty.
   */
  children?: ReactNode;
  /** An icon for a neutral chip. The other variants have their own. */
  icon?: Icon;
  /** A date chip: the machine-readable date for the `time` element. */
  dateTime?: string;
  /** A date chip: the date is past. The chip says so in words, not only by color. */
  overdue?: boolean;
  /** Makes the chip a button. A static chip has no handler. */
  onPress?: () => void;
}

function iconOf(variant: ChipVariant, state: KnowledgeStateName | undefined, overdue: boolean) {
  if (state !== undefined) {
    return KNOWLEDGE_ICONS[state];
  }
  if (variant === "date") {
    return overdue ? IconCalendarExclamation : IconCalendar;
  }
  return variant === "workstream" ? IconStack2 : undefined;
}

const KNOWLEDGE = { proposal: "proposed", assumption: "assumption", unknown: "unknown" } as const;

/**
 * A small fact on a card or a row: a date, a workstream, a state of knowledge (doc/design/components.md).
 * 20 px high, 12 px text, 12 px icon (ADR 0082). An interactive chip is a button whose hit area,
 * not the chip, grows to 44 px on a coarse pointer.
 */
export function Chip({
  variant = "neutral",
  children,
  icon,
  dateTime,
  overdue = false,
  onPress,
}: ChipProps) {
  const state = variant in KNOWLEDGE ? KNOWLEDGE[variant as keyof typeof KNOWLEDGE] : undefined;
  const Icon = icon ?? iconOf(variant, state, overdue);
  const content =
    state !== undefined ? (
      <>
        {t(`knowledge-${state}`)}
        {children ? ` · ${children}` : null}
      </>
    ) : variant === "date" ? (
      <>
        <time dateTime={dateTime}>{children}</time>
        {overdue && <span className={styles.hidden}>, {t("chip-overdue")}</span>}
      </>
    ) : variant === "workstream" ? (
      <>
        <span className={styles.hidden}>{t("chip-workstream")}: </span>
        {children}
      </>
    ) : (
      children
    );
  const common = {
    className: styles.chip,
    "data-variant": variant,
    "data-overdue": overdue || undefined,
    "data-interactive": onPress ? true : undefined,
  };
  const body = (
    <>
      {Icon && <Icon className={styles.icon} size={12} stroke={1.5} aria-hidden="true" />}
      <span className={styles.text}>{content}</span>
    </>
  );
  return onPress ? (
    <AriaButton {...common} onPress={onPress}>
      {body}
    </AriaButton>
  ) : (
    <span {...common}>{body}</span>
  );
}
