import { type Icon, IconCheck } from "@tabler/icons-react";
import type { ReactNode } from "react";
import {
  Button,
  Menu,
  MenuItem,
  MenuTrigger,
  Popover,
  type PopoverProps,
  Separator,
} from "react-aria-components";
import styles from "./MenuButton.module.css";
import popover from "./Popover.module.css";

export interface MenuButtonItem {
  id: string;
  label: string;
  icon?: Icon;
  /** The current choice of a list, for example the current event. */
  current?: boolean;
  isDisabled?: boolean;
  /** A line above the item. */
  separated?: boolean;
}

export interface MenuButtonProps {
  /** The content of the trigger: an avatar, a name, an icon. */
  trigger: ReactNode;
  /** The accessible name of the trigger button. */
  triggerLabel: string;
  triggerClassName: string | undefined;
  /** The accessible name of the menu. */
  label: string;
  items: MenuButtonItem[];
  onAction: (id: string) => void;
  onOpen?: () => void;
  placement?: PopoverProps["placement"];
}

/**
 * A menu whose trigger the caller draws: the member menu, the event switcher and „Mehr“ of the shell.
 * `Menu` has a text button; these triggers show an avatar, a name with a chevron or a stacked icon.
 * The caller gives the class of the trigger, which needs `[data-hovered]` and `[data-focus-visible]`.
 */
export function MenuButton({
  trigger,
  triggerLabel,
  triggerClassName,
  label,
  items,
  onAction,
  onOpen,
  placement,
}: MenuButtonProps) {
  return (
    <MenuTrigger
      onOpenChange={(open) => {
        if (open) {
          onOpen?.();
        }
      }}
    >
      <Button className={triggerClassName} aria-label={triggerLabel}>
        {trigger}
      </Button>
      <Popover className={popover.popover} {...(placement ? { placement } : {})}>
        <Menu
          className={popover.list}
          aria-label={label}
          onAction={(key) => onAction(String(key))}
          disabledKeys={items.filter((item) => item.isDisabled).map((item) => item.id)}
        >
          {items.flatMap((item) => [
            ...(item.separated
              ? [<Separator key={`${item.id}-line`} className={styles.line} />]
              : []),
            <MenuItem
              key={item.id}
              id={item.id}
              textValue={item.label}
              className={popover.item}
              {...(item.current ? { "aria-current": "true" as const } : {})}
            >
              {item.icon && (
                <item.icon size={16} stroke={1.5} aria-hidden="true" className={styles.icon} />
              )}
              <span className={styles.label}>{item.label}</span>
              {item.current && <IconCheck size={14} aria-hidden="true" className={styles.check} />}
            </MenuItem>,
          ])}
        </Menu>
      </Popover>
    </MenuTrigger>
  );
}
