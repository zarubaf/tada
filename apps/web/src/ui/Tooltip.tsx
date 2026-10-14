import type { ReactNode } from "react";
import {
  Tooltip as AriaTooltip,
  type TooltipProps as AriaTooltipProps,
  TooltipTrigger as AriaTooltipTrigger,
} from "react-aria-components";
import styles from "./Tooltip.module.css";

export interface TooltipProps {
  /** The one focusable element that the tooltip describes. */
  children: ReactNode;
  /** Text only, no interactive content. */
  text: string;
  placement?: AriaTooltipProps["placement"];
}

/**
 * A short text that opens on hover and on keyboard focus (doc/design/components.md). It repeats a
 * name or adds a hint; no information lives only here, because touch has no hover.
 */
export function Tooltip({ children, text, placement = "bottom" }: TooltipProps) {
  return (
    <AriaTooltipTrigger delay={500} closeDelay={0}>
      {children}
      <AriaTooltip className={styles.tooltip} placement={placement} offset={6}>
        {text}
      </AriaTooltip>
    </AriaTooltipTrigger>
  );
}
