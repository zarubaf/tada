import type { Icon } from "@tabler/icons-react";
import type { ReactNode, Ref } from "react";
import {
  Menu as AriaMenu,
  MenuItem,
  MenuTrigger,
  Popover,
  type PopoverProps,
} from "react-aria-components";
import { Button, type ButtonProps } from "./Button";
import menu from "./MenuButton.module.css";
import popover from "./Popover.module.css";

export interface MenuProps {
  /** The text of the trigger button. */
  trigger: string;
  /** The accessible name of the menu. */
  label: string;
  items: { id: string; label: string; icon?: ReactNode }[];
  onAction: (id: string) => void;
  placement?: PopoverProps["placement"];
  /** The look of the trigger, for a toolbar: the same props as `Button`. */
  variant?: ButtonProps["variant"];
  size?: ButtonProps["size"];
  /** An icon before the text of the trigger. */
  triggerIcon?: Icon;
  /** The trigger button, for a caller that returns focus to it. */
  ref?: Ref<HTMLButtonElement>;
}

/** A button that opens a list of actions (doc/design/components.md). */
export function Menu({
  trigger,
  label,
  items,
  onAction,
  placement,
  variant,
  size,
  triggerIcon: TriggerIcon,
  ref,
}: MenuProps) {
  return (
    <MenuTrigger>
      <Button ref={ref} {...(variant && { variant })} {...(size && { size })}>
        {TriggerIcon && <TriggerIcon size={14} stroke={1.5} aria-hidden="true" />}
        {trigger}
      </Button>
      <Popover className={popover.popover} {...(placement ? { placement } : {})}>
        <AriaMenu
          className={popover.list}
          aria-label={label}
          onAction={(key) => onAction(String(key))}
        >
          {items.map((item) => (
            <MenuItem key={item.id} id={item.id} textValue={item.label} className={popover.item}>
              {item.icon && <span className={popover.slot}>{item.icon}</span>}
              <span className={menu.label}>{item.label}</span>
            </MenuItem>
          ))}
        </AriaMenu>
      </Popover>
    </MenuTrigger>
  );
}
