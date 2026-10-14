import type { Icon } from "@tabler/icons-react";
import type { Ref } from "react";
import { Button as AriaButton, type ButtonProps as AriaButtonProps } from "react-aria-components";
import styles from "./IconButton.module.css";
import { Tooltip } from "./Tooltip";

export interface IconButtonProps
  extends Omit<AriaButtonProps, "className" | "style" | "children" | "aria-label"> {
  icon: Icon;
  /**
   * What the button does, for example „Schliessen“. It is the accessible name and the tooltip.
   * Required: an icon alone has no name.
   */
  label: string;
  ref?: Ref<HTMLButtonElement>;
}

/**
 * A square button with an icon, for toolbars and rows (doc/design/components.md). 32 px, and 44 px
 * with a coarse pointer, as `--control-height-md` sets. The label is the accessible name and the
 * tooltip, which also opens on keyboard focus, unlike the native `title`.
 */
export function IconButton({ icon: Icon, label, ...props }: IconButtonProps) {
  return (
    <Tooltip text={label}>
      <AriaButton {...props} className={styles.button} aria-label={label}>
        <Icon size={16} stroke={1.5} aria-hidden="true" />
      </AriaButton>
    </Tooltip>
  );
}
