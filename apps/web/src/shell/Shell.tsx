// The app shell (doc/design/layout-and-responsiveness.md): a sidebar on medium and wide layouts, a
// top bar and a bottom bar on narrow layouts. It shows only the items that exist.
import { Fragment, type ReactNode, useState } from "react";
import {
  Button,
  Label,
  ListBox,
  ListBoxItem,
  Menu,
  MenuItem,
  MenuTrigger,
  Popover,
  Select,
  SelectValue,
} from "react-aria-components";
import { type Api, problemMessage } from "../api/client";
import { t } from "../i18n";
import { usePathname } from "../router/Router";
import { CHOOSE_ORGANIZATION_PATH, isPublicPath } from "../session/paths";
import { useOptionalSession } from "../session/SessionProvider";
import { LiveRegion } from "../ui/LiveRegion";
import { NavLink } from "../ui/NavLink";
import styles from "./Shell.module.css";

export function Shell({ api, children }: { api: Api; children: ReactNode }) {
  const session = useOptionalSession();
  const [failure, setFailure] = useState<string>();
  const pathname = usePathname();
  const frameless = isPublicPath(pathname) || pathname === CHOOSE_ORGANIZATION_PATH;

  // Without a session or an organization, and on the pages of the sign-in, there is no frame.
  if (frameless || !session?.organization) {
    return children;
  }
  const { user, organization, memberships, refresh, signOut } = session;

  const choose = async (organizationId: string) => {
    try {
      const { error } = await api.POST("/api/v1/session/organization", {
        body: { organization_id: organizationId },
      });
      setFailure(error ? problemMessage(error) : undefined);
      if (!error) {
        await refresh();
      }
    } catch {
      setFailure(problemMessage(undefined));
    }
  };

  return (
    <div className={styles.shell}>
      <aside className={styles.side} aria-label={t("shell-side")}>
        <p className={styles.brand}>{t("app-name")}</p>
        {memberships.length > 1 && (
          <Select
            className={styles.switcher}
            selectedKey={organization.organization_id}
            onSelectionChange={(key) => void choose(String(key))}
          >
            <Label className={styles.switcherLabel}>{t("organization-switcher")}</Label>
            <Button className={styles.select}>
              <SelectValue />
            </Button>
            <Popover className={styles.popover}>
              <ListBox className={styles.menu}>
                {memberships.map((membership) => (
                  <ListBoxItem
                    key={membership.organization_id}
                    id={membership.organization_id}
                    className={styles.item}
                  >
                    {membership.name}
                  </ListBoxItem>
                ))}
              </ListBox>
            </Popover>
          </Select>
        )}
        <nav className={styles.nav} aria-label={t("shell-nav")}>
          <NavLink to="/events" large>
            {t("nav-events")}
          </NavLink>
          <NavLink to="/settings/members" within="/settings" large>
            {t("nav-settings")}
          </NavLink>
        </nav>
        <MenuTrigger>
          <Button className={styles.member}>{user.displayName}</Button>
          <Popover className={styles.popover} placement="top end">
            <Menu
              className={styles.menu}
              aria-label={t("member-menu")}
              onAction={() => void signOut().then(setFailure)}
            >
              <MenuItem className={styles.item}>{t("sign-out")}</MenuItem>
            </Menu>
          </Popover>
        </MenuTrigger>
        <LiveRegion kind="alert" className={styles.failure}>
          {failure}
        </LiveRegion>
      </aside>
      <Fragment key={organization.organization_id}>{children}</Fragment>
    </div>
  );
}
