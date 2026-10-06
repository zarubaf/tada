import { Button as AriaButton, type ButtonProps as AriaButtonProps } from "react-aria-components";
import styles from "./Button.module.css";

export interface ButtonProps extends Omit<AriaButtonProps, "className" | "style"> {
  variant?: "primary" | "secondary";
}

/** A button. At most one primary button in a view (doc/design/components.md). */
export function Button({ variant = "secondary", ...props }: ButtonProps) {
  return <AriaButton {...props} className={styles.button} data-variant={variant} />;
}
