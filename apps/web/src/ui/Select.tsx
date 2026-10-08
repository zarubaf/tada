import {
  Select as AriaSelect,
  Button,
  Label,
  ListBox,
  ListBoxItem,
  Popover,
  SelectValue,
} from "react-aria-components";
import styles from "./Select.module.css";

export interface SelectProps {
  label: string;
  options: { id: string; label: string }[];
  /** The ID of the selected option, or nothing. */
  value: string | undefined;
  onChange: (id: string) => void;
  placeholder?: string;
  isDisabled?: boolean;
  /** Only the accessible name shows the label, for a select in a table row. */
  labelHidden?: boolean;
}

/** A select for up to seven options (doc/design/components.md). */
export function Select({
  label,
  options,
  value,
  onChange,
  placeholder,
  isDisabled,
  labelHidden,
}: SelectProps) {
  return (
    <AriaSelect
      className={styles.select}
      selectedKey={value ?? null}
      onSelectionChange={(key) => onChange(String(key))}
      isDisabled={isDisabled}
      {...(labelHidden ? { "aria-label": label } : {})}
      {...(placeholder === undefined ? {} : { placeholder })}
    >
      {!labelHidden && <Label className={styles.label}>{label}</Label>}
      <Button className={styles.trigger}>
        <SelectValue />
        <span aria-hidden="true">▾</span>
      </Button>
      <Popover className={styles.popover}>
        <ListBox className={styles.menu}>
          {options.map((option) => (
            <ListBoxItem key={option.id} id={option.id} className={styles.item}>
              {option.label}
            </ListBoxItem>
          ))}
        </ListBox>
      </Popover>
    </AriaSelect>
  );
}
