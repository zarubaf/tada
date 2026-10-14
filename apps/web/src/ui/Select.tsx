import { IconChevronDown } from "@tabler/icons-react";
import {
  Button as AriaButton,
  Select as AriaSelect,
  FieldError,
  Label,
  ListBox,
  ListBoxItem,
  Popover,
  SelectValue,
} from "react-aria-components";
import { Button } from "./Button";
import popover from "./Popover.module.css";
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
  /** The error below the select. It marks the select as invalid. */
  error?: string | undefined;
  /**
   * `toolbar` looks like the other controls of a toolbar, such as `SearchSelect`: the look of a
   * button and a chevron. The default is the field of a form.
   */
  variant?: "field" | "toolbar";
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
  error,
  variant = "field",
}: SelectProps) {
  return (
    <AriaSelect
      className={styles.select}
      selectedKey={value ?? null}
      onSelectionChange={(key) => onChange(String(key))}
      isDisabled={isDisabled}
      isInvalid={error !== undefined || undefined}
      {...(labelHidden ? { "aria-label": label } : {})}
      {...(placeholder === undefined ? {} : { placeholder })}
    >
      {!labelHidden && <Label className={styles.label}>{label}</Label>}
      {variant === "toolbar" ? (
        <Button>
          <SelectValue className={styles.value} />
          <IconChevronDown size={14} aria-hidden="true" />
        </Button>
      ) : (
        <AriaButton className={styles.trigger}>
          <SelectValue />
          <span aria-hidden="true">▾</span>
        </AriaButton>
      )}
      {error && <FieldError className={styles.error}>{error}</FieldError>}
      <Popover className={popover.popover}>
        <ListBox className={popover.list}>
          {options.map((option) => (
            <ListBoxItem key={option.id} id={option.id} className={popover.item}>
              {option.label}
            </ListBoxItem>
          ))}
        </ListBox>
      </Popover>
    </AriaSelect>
  );
}
