import { Checkbox as AriaCheckbox } from "react-aria-components";
import styles from "./Checkbox.module.css";

export interface CheckboxProps {
  /** The label to the right of the box (doc/design/components.md). */
  label: string;
  isSelected: boolean;
  onChange: (selected: boolean) => void;
}

/** A checkbox with its label to the right. */
export function Checkbox({ label, isSelected, onChange }: CheckboxProps) {
  return (
    <AriaCheckbox className={styles.checkbox} isSelected={isSelected} onChange={onChange}>
      <span className={styles.box} aria-hidden="true">
        {isSelected ? "✓" : ""}
      </span>
      {label}
    </AriaCheckbox>
  );
}
