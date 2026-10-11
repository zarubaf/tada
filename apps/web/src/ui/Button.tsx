import type { Ref } from "react";
import { Button as AriaButton, type ButtonProps as AriaButtonProps } from "react-aria-components";
import styles from "./Button.module.css";

export interface ButtonProps extends Omit<AriaButtonProps, "className" | "style"> {
  /**
   * `danger` is for a destructive action before its confirmation step. `quiet` is for a minor
   * action, for example the version in the footer of the sidebar.
   */
  variant?: "primary" | "secondary" | "quiet" | "danger";
  /** For a page that returns focus to the button. */
  ref?: Ref<HTMLButtonElement>;
}

/** A button. At most one primary button in a view (doc/design/components.md). */
export function Button({ variant = "secondary", ...props }: ButtonProps) {
  return <AriaButton {...props} className={styles.button} data-variant={variant} />;
}
