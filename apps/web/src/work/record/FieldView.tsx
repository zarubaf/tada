import { IconCalendar, IconCalendarExclamation } from "@tabler/icons-react";
import { isOverdue } from "../../facts/dueDate";
import { formatDate } from "../../facts/formatValue";
import { t } from "../../i18n";
import { Chip } from "../../ui/Chip";
import { Redacted } from "../../ui/Redacted";
import { mayBeOverdue, type WorkItem } from "../workItem";
import type { Choices } from "./choices";
import { type FieldSpec, type FieldValue, peopleIds } from "./descriptor";
import styles from "./RecordPanel.module.css";

/** The words for a field without a value. */
export function Empty({ spec }: { spec: FieldSpec }) {
  return <span className={styles.empty}>{spec.emptyKey ? t(spec.emptyKey) : ""}</span>;
}

/**
 * A date as text with its icon: the panel shows it as a value among values, not as a chip among
 * chips. An overdue date is in the danger color and says „überfällig“ in words.
 */
function DateValue({ date, today, overdue }: { date: string; today: string; overdue: boolean }) {
  const late = overdue && isOverdue(date, today);
  const Icon = late ? IconCalendarExclamation : IconCalendar;
  return (
    <span className={styles.date} data-overdue={late || undefined}>
      <Icon size={14} stroke={1.5} aria-hidden="true" />
      <time dateTime={date}>{formatDate(date)}</time>
      {late && <span className={styles.hidden}>, {t("overdue")}</span>}
    </span>
  );
}

/**
 * A value as text: what the member sees when the field is not being edited, and all that they see
 * of a field they may not change. A redacted record shows the tombstone for its text fields.
 */
export function FieldView({
  spec,
  value,
  choices,
  item,
  today,
}: {
  spec: FieldSpec;
  value: FieldValue;
  choices: Choices;
  item: WorkItem;
  today: string;
}) {
  const isText = spec.control === "text" || spec.control === "multiline";
  if (isText && item.redacted) {
    return <Redacted />;
  }
  if (value === null || value === "") {
    return <Empty spec={spec} />;
  }
  switch (spec.control) {
    case "date":
      return (
        <DateValue
          date={value}
          today={today}
          overdue={spec.overdue === true && mayBeOverdue(item)}
        />
      );
    case "multiline":
      return <span className={styles.text}>{value}</span>;
    case "text":
      return <>{value}</>;
    case "people":
      return (
        <ul className={styles.people}>
          {peopleIds(value).map((id) => {
            const person = choices.options.find((option) => option.id === id);
            return (
              <li key={id} className={styles.value}>
                {person?.leading}
                <span className={styles.valueName} title={person?.label}>
                  {person?.label ?? ""}
                </span>
              </li>
            );
          })}
        </ul>
      );
    case "area":
      return <Chip variant="workstream">{label(choices, value)}</Chip>;
    default: {
      const chosen = choices.options.find((option) => option.id === value);
      return (
        <span className={styles.value}>
          {chosen?.leading}
          <span className={styles.valueName} title={chosen?.label}>
            {chosen?.label ?? ""}
          </span>
        </span>
      );
    }
  }
}

function label(choices: Choices, value: string): string {
  return choices.options.find((option) => option.id === value)?.label ?? "";
}
