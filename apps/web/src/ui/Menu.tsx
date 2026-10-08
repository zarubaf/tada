import {
  Menu as AriaMenu,
  MenuItem,
  MenuTrigger,
  Popover,
  type PopoverProps,
} from "react-aria-components";
import { Button } from "./Button";
import popover from "./Popover.module.css";

export interface MenuProps {
  /** The text of the trigger button. */
  trigger: string;
  /** The accessible name of the menu. */
  label: string;
  items: { id: string; label: string }[];
  onAction: (id: string) => void;
  placement?: PopoverProps["placement"];
}

/** A button that opens a list of actions (doc/design/components.md). */
export function Menu({ trigger, label, items, onAction, placement }: MenuProps) {
  return (
    <MenuTrigger>
      <Button>{trigger}</Button>
      <Popover className={popover.popover} {...(placement ? { placement } : {})}>
        <AriaMenu
          className={popover.list}
          aria-label={label}
          onAction={(key) => onAction(String(key))}
        >
          {items.map((item) => (
            <MenuItem key={item.id} id={item.id} className={popover.item}>
              {item.label}
            </MenuItem>
          ))}
        </AriaMenu>
      </Popover>
    </MenuTrigger>
  );
}
