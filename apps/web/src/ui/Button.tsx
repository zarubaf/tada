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
  /**
   * A press is under way. The button keeps its look, refuses the press and the submit, and is
   * `aria-disabled`. React Aria's own `isPending` is not used: it announces the start and the end
   * of the state through an element that points at the button, and when the page replaces the
   * button before the announcement ends (a form that made its record), the pointer leads nowhere
   * (axe: role-img-alt). The action that the button starts announces its result.
   */
  isPending?: boolean;
  /** For a page that returns focus to the button. */
  ref?: Ref<HTMLButtonElement>;
}

/** A button. At most one primary button in a view (doc/design/components.md). */
export function Button({
  variant = "secondary",
  size = "md",
  isRefused,
  isPending,
  onPress,
  ...props
}: ButtonProps) {
  return (
    <AriaButton
      {...props}
      {...(isPending && props.type === "submit" && { type: "button" })}
      {...(isRefused ? { "aria-disabled": true, "data-refused": "" } : {})}
      {...(isPending ? { "aria-disabled": true, "data-pending": "" } : {})}
      {...(isRefused || isPending ? {} : { onPress })}
      className={styles.button}
      data-variant={variant}
      data-size={size}
    />
  );
}
