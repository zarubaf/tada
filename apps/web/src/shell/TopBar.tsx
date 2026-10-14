// The top bar of 48 px on all widths (ADR 0082): the toggle of the sidebar, the plain title of the
// context, the command box, the theme toggle and the member menu. Each page keeps its own `h1`.
import { IconLayoutSidebar, IconSearch } from "@tabler/icons-react";
import { t } from "../i18n";
import { Avatar } from "../ui/Avatar";
import { IconButton } from "../ui/IconButton";
import { Kbd } from "../ui/Kbd";
import { MenuButton, type MenuButtonItem } from "../ui/MenuButton";
import { ThemeToggle } from "../ui/ThemeToggle";
import { paletteHint } from "./shortcut";
import styles from "./TopBar.module.css";

export interface TopBarProps {
  /** The event name or the section: plain text. */
  title: string;
  /** What the toggle does now, for example „Seitenleiste ausblenden“. */
  toggleLabel: string;
  /** Whether the sidebar or the drawer is open. */
  expanded: boolean;
  onToggle: () => void;
  onOpenPalette: () => void;
  member: { id: string; name: string };
  memberItems: MenuButtonItem[];
  onMemberAction: (id: string) => void;
}

export function TopBar({
  title,
  toggleLabel,
  expanded,
  onToggle,
  onOpenPalette,
  member,
  memberItems,
  onMemberAction,
}: TopBarProps) {
  return (
    <header className={styles.bar}>
      <IconButton
        icon={IconLayoutSidebar}
        label={toggleLabel}
        aria-expanded={expanded}
        onPress={onToggle}
      />
      <p className={styles.title} title={title}>
        {title}
      </p>
      <button
        type="button"
        className={styles.command}
        aria-label={t("command-box-label")}
        aria-keyshortcuts="Control+K Meta+K"
        aria-haspopup="dialog"
        onClick={onOpenPalette}
      >
        <IconSearch size={16} stroke={1.5} aria-hidden="true" />
        <span className={styles.commandText}>{t("command-box")}</span>
        <span className={styles.hint} aria-hidden="true">
          <Kbd>{paletteHint()}</Kbd>
        </span>
      </button>
      <ThemeToggle iconOnly />
      <MenuButton
        trigger={<Avatar name={member.name} id={member.id} size={24} decorative />}
        triggerLabel={t("member-menu-trigger", { name: member.name })}
        triggerClassName={styles.member}
        label={t("member-menu")}
        items={memberItems}
        onAction={onMemberAction}
        placement="bottom end"
      />
    </header>
  );
}
