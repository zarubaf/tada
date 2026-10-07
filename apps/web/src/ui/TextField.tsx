import {
  TextField as AriaTextField,
  Input,
  Label,
  type TextFieldProps,
} from "react-aria-components";
import styles from "./TextField.module.css";

export interface Props extends Omit<TextFieldProps, "className" | "style" | "children"> {
  label: string;
}

/** A text field with the label above it (doc/design/components.md). */
export function TextField({ label, ...props }: Props) {
  return (
    <AriaTextField {...props} className={styles.field}>
      <Label className={styles.label}>{label}</Label>
      <Input className={styles.input} />
    </AriaTextField>
  );
}
