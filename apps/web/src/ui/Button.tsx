import type { Ref } from "react";
import { Button as AriaButton, type ButtonProps as AriaButtonProps } from "react-aria-components";
import styles from "./Button.module.css";

export interface ButtonProps extends Omit<AriaButtonProps, "className" | "style"> {
  /**
   * `danger` is for a destructive action before its confirmation step. `quiet` is for a minor
   * action, for example the version in the footer of the sidebar.
   */
  variant?: "primary" | "secondary" | "quiet" | "danger";
  /** `sm` is the compact button of a toolbar row; it is 44 px high again with a coarse pointer. */
  size?: "md" | "sm";
  /**
   * The button looks off and refuses the press, but stays in the tab order (`aria-disabled`), so
   * that focus is not lost and a description can say why. Unlike `isDisabled` it is not removed
   * from the keyboard.
   */
  isRefused?: boolean;
  /** For a page that returns focus to the button. */
  ref?: Ref<HTMLButtonElement>;
}

/** A button. At most one primary button in a view (doc/design/components.md). */
export function Button({
  variant = "secondary",
  size = "md",
  isRefused,
  onPress,
  ...props
}: ButtonProps) {
  return (
    <AriaButton
      {...props}
      {...(isRefused ? { "aria-disabled": true, "data-refused": "" } : { onPress })}
      className={styles.button}
      data-variant={variant}
      data-size={size}
    />
  );
}
