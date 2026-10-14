import { type Icon, IconCalendar, IconCalendarExclamation, IconStack2 } from "@tabler/icons-react";
import type { ReactNode } from "react";
import { Button as AriaButton } from "react-aria-components";
import { isOverdue, todayLocal } from "../facts/dueDate";
import { formatDate } from "../facts/formatValue";
import { t } from "../i18n";
import styles from "./Chip.module.css";
import { KNOWLEDGE_ICONS, type KnowledgeStateName } from "./KnowledgeState";

export type ChipVariant = "neutral" | "date" | "workstream" | "proposal" | "assumption" | "unknown";

interface ChipBase {
  /** Makes the chip a button. A static chip has no handler. */
  onPress?: () => void;
}

/**
 * A date. The chip formats it and says „überfällig“ in words when it lies before `today`, so no
 * caller keeps the rule. A date chip has no children and no icon: the date and its icon follow.
 */
export interface DateChipProps extends ChipBase {
  variant: "date";
  /** A civil date, for example `2026-10-15`. */
  date: string;
  /** The civil date of today. Default: the day of the browser. */
  today?: string;
  /**
   * Overrides the rule „before today“, for a record that is closed (done, canceled) or that has
   * no due date, such as the day of a decision. `false` never shows the overdue state.
   */
  overdue?: boolean;
}

export interface TextChipProps extends ChipBase {
  variant?: Exclude<ChipVariant, "date">;
  /**
   * The text. For a proposal, an assumption or an unknown value it is the detail after the label,
   * for example the proposed date, and may be empty.
   */
  children?: ReactNode;
  /** An icon for a neutral chip. The other variants have their own. */
  icon?: Icon;
}

export type ChipProps = DateChipProps | TextChipProps;

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
export function Chip(props: ChipProps) {
  const { variant = "neutral", onPress } = props;
  const overdue =
    props.variant === "date" &&
    props.overdue !== false &&
    isOverdue(props.date, props.today ?? todayLocal());
  const state = variant in KNOWLEDGE ? KNOWLEDGE[variant as keyof typeof KNOWLEDGE] : undefined;
  const Icon =
    (props.variant === "date" ? undefined : props.icon) ?? iconOf(variant, state, overdue);
  const children = props.variant === "date" ? undefined : props.children;
  const content =
    props.variant === "date" ? (
      <>
        <time dateTime={props.date}>{formatDate(props.date)}</time>
        {overdue && <span className={styles.hidden}>, {t("overdue")}</span>}
      </>
    ) : state !== undefined ? (
      <>
        {t(`knowledge-${state}`)}
        {children ? ` · ${children}` : null}
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
