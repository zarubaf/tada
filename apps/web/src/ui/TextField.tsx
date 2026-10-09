import {
  TextField as AriaTextField,
  FieldError,
  Input,
  Label,
  Text,
  TextArea,
  type TextFieldProps,
} from "react-aria-components";
import styles from "./TextField.module.css";

export interface Props extends Omit<TextFieldProps, "className" | "style" | "children"> {
  label: string;
  /** Help below the input. */
  help?: string;
  /** The error below the help. It marks the field as invalid. */
  error?: string | undefined;
  /** Monospace text, for keys. */
  mono?: boolean;
  /** Several lines, for a text such as a notice. The field grows on request. */
  rows?: number;
}

/**
 * A text field: label above, help below, error below the help (doc/design/components.md).
 * With `rows` it is a text area.
 */
export function TextField({ label, help, error, mono, rows, ...props }: Props) {
  return (
    <AriaTextField {...props} className={styles.field} isInvalid={error !== undefined || undefined}>
      <Label className={styles.label}>{label}</Label>
      {rows ? (
        <TextArea className={`${styles.input} ${styles.area}`} rows={rows} />
      ) : (
        <Input className={styles.input} data-mono={mono || undefined} />
      )}
      {help && (
        <Text slot="description" className={styles.help}>
          {help}
        </Text>
      )}
      {error && <FieldError className={styles.error}>{error}</FieldError>}
    </AriaTextField>
  );
}
