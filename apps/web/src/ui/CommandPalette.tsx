import type { Icon } from "@tabler/icons-react";
import { IconSearch } from "@tabler/icons-react";
import { useState } from "react";
import {
  Autocomplete,
  Dialog,
  Header,
  Input,
  Menu,
  MenuItem,
  MenuSection,
  Modal,
  ModalOverlay,
  SearchField,
  useFilter,
} from "react-aria-components";
import { t } from "../i18n";
import styles from "./CommandPalette.module.css";
import { LiveRegion } from "./LiveRegion";

export interface Command {
  id: string;
  label: string;
  icon?: Icon;
}

export interface CommandGroup {
  id: string;
  label: string;
  commands: Command[];
}

export interface CommandPaletteProps {
  /** The accessible name of the dialog, for example „Befehle“. */
  label: string;
  placeholder: string;
  groups: CommandGroup[];
  /** Called with the ID of the chosen command. */
  onAction: (id: string) => void;
  /** Called by Escape and by a press outside the palette. */
  onClose: () => void;
}

/**
 * A search field above a list of commands, in a dialog near the top of the page (doc/design/components.md).
 * It is open while it is mounted. The typed text filters the commands; a live region announces the
 * number of results. Arrow keys move through the list while focus stays in the field, and Enter
 * runs the chosen command.
 */
export default function CommandPalette({
  label,
  placeholder,
  groups,
  onAction,
  onClose,
}: CommandPaletteProps) {
  const [text, setText] = useState("");
  const { contains } = useFilter({ sensitivity: "base" });
  const shown = groups
    .map((group) => ({
      ...group,
      commands: group.commands.filter((command) => contains(command.label, text)),
    }))
    .filter((group) => group.commands.length > 0);
  const count = shown.reduce((sum, group) => sum + group.commands.length, 0);

  return (
    <ModalOverlay
      className={styles.overlay}
      isOpen
      isDismissable
      onOpenChange={(open) => {
        if (!open) {
          onClose();
        }
      }}
    >
      <Modal className={styles.modal}>
        <Dialog className={styles.dialog} aria-label={label}>
          <Autocomplete inputValue={text} onInputChange={setText}>
            <SearchField className={styles.search} aria-label={placeholder} autoFocus>
              <IconSearch size={16} stroke={1.5} aria-hidden="true" className={styles.icon} />
              <Input className={styles.input} placeholder={placeholder} />
            </SearchField>
            <Menu
              className={styles.list}
              aria-label={label}
              onAction={(key) => onAction(String(key))}
              renderEmptyState={() => <p className={styles.empty}>{t("combobox-empty")}</p>}
            >
              {shown.map((group) => (
                <MenuSection key={group.id} className={styles.section}>
                  <Header className={styles.group}>{group.label}</Header>
                  {group.commands.map((command) => (
                    <MenuItem
                      key={command.id}
                      id={command.id}
                      textValue={command.label}
                      className={styles.item}
                    >
                      {command.icon && (
                        <command.icon
                          size={16}
                          stroke={1.5}
                          aria-hidden="true"
                          className={styles.itemIcon}
                        />
                      )}
                      {command.label}
                    </MenuItem>
                  ))}
                </MenuSection>
              ))}
            </Menu>
          </Autocomplete>
          <LiveRegion kind="status" visuallyHidden>
            {count === 0 ? t("combobox-empty") : t("command-results", { count })}
          </LiveRegion>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
