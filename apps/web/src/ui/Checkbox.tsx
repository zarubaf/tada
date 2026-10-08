import { Checkbox as AriaCheckbox } from "react-aria-components";
import styles from "./Checkbox.module.css";

export interface CheckboxProps {
  /** The label to the right of the box (doc/design/components.md). */
  label: string;
  isSelected: boolean;
  onChange: (selected: boolean) => void;
  isDisabled?: boolean;
  /** Only the accessible name shows the label, for a checkbox in a row or a card header. */
  labelHidden?: boolean;
}

/** A checkbox with its label to the right. */
export function Checkbox({ label, isSelected, onChange, isDisabled, labelHidden }: CheckboxProps) {
  return (
    <AriaCheckbox
      className={styles.checkbox}
      isSelected={isSelected}
      onChange={onChange}
      isDisabled={isDisabled}
    >
      <span className={styles.box} aria-hidden="true">
        {isSelected ? "✓" : ""}
      </span>
      {labelHidden ? <span className={styles.hidden}>{label}</span> : label}
    </AriaCheckbox>
  );
}
