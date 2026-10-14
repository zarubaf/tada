import type { Icon } from "@tabler/icons-react";
import { Menu, MenuItem, MenuTrigger, Popover } from "react-aria-components";
import { IconButton } from "./IconButton";
import menu from "./MenuButton.module.css";
import popover from "./Popover.module.css";

export interface IconMenuItem {
  id: string;
  label: string;
  icon?: Icon;
}

export interface IconMenuProps {
  /** The icon of the trigger. */
  icon: Icon;
  /** The accessible name of the trigger button and of the menu. */
  label: string;
  items: readonly IconMenuItem[];
  onAction: (id: string) => void;
  size?: "sm" | "md";
}

/**
 * A menu behind an icon button, for the actions of a record in the head of a panel
 * (doc/design/components.md). The button is an `IconButton`, so it has the name, the tooltip and
 * the touch target of one; the list opens under its right edge.
 */
export function IconMenu({ icon, label, items, onAction, size = "sm" }: IconMenuProps) {
  return (
    <MenuTrigger>
      <IconButton icon={icon} label={label} size={size} />
      <Popover className={popover.popover} placement="bottom end">
        <Menu className={popover.list} aria-label={label} onAction={(key) => onAction(String(key))}>
          {items.map((item) => (
            <MenuItem key={item.id} id={item.id} textValue={item.label} className={popover.item}>
              {item.icon && (
                <item.icon size={16} stroke={1.5} aria-hidden="true" className={menu.icon} />
              )}
              <span className={menu.label}>{item.label}</span>
            </MenuItem>
          ))}
        </Menu>
      </Popover>
    </MenuTrigger>
  );
}
