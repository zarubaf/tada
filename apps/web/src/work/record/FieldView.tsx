import { t } from "../../i18n";
import { Chip } from "../../ui/Chip";
import { Redacted } from "../../ui/Redacted";
import { mayBeOverdue, type WorkItem } from "../workItem";
import type { Choices } from "./choices";
import type { FieldSpec, FieldValue } from "./descriptor";
import styles from "./RecordPanel.module.css";

/** The words for a field without a value. */
export function Empty({ spec }: { spec: FieldSpec }) {
  return <span className={styles.empty}>{spec.emptyKey ? t(spec.emptyKey) : ""}</span>;
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
        <Chip
          variant="date"
          date={value}
          today={today}
          overdue={spec.overdue === true && mayBeOverdue(item)}
        />
      );
    case "multiline":
      return <span className={styles.text}>{value}</span>;
    case "text":
      return <>{value}</>;
    case "area":
      return <Chip variant="workstream">{label(choices, value)}</Chip>;
    default: {
      const chosen = choices.options.find((option) => option.id === value);
      return (
        <span className={styles.value}>
          {chosen?.leading}
          <span>{chosen?.label ?? ""}</span>
        </span>
      );
    }
  }
}

function label(choices: Choices, value: string): string {
  return choices.options.find((option) => option.id === value)?.label ?? "";
}
