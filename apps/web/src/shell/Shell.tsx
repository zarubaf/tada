// The app shell (doc/design/layout-and-responsiveness.md): a sidebar on medium and wide layouts, a
// top bar and a bottom bar on narrow layouts. It shows only the items that exist.
import { Fragment, type ReactNode, useState } from "react";
import { Button, Menu, MenuItem, MenuTrigger, Popover } from "react-aria-components";
import { type Api, problemMessage } from "../api/client";
import { t } from "../i18n";
import { Link } from "../router/Router";
import { useSession } from "../session/SessionProvider";
import styles from "./Shell.module.css";

export function Shell({ api, children }: { api: Api; children: ReactNode }) {
  const { user, organization, memberships, refresh, signOut } = useSession();
  const [failure, setFailure] = useState<string>();

  // Without an organization the member chooses one first; that page needs no frame.
  if (!organization) {
    return children;
  }

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
          <label className={styles.switcher}>
            <span className={styles.switcherLabel}>{t("organization-switcher")}</span>
            <select
              className={styles.select}
              value={organization.organization_id}
              onChange={(event) => void choose(event.target.value)}
            >
              {memberships.map((membership) => (
                <option key={membership.organization_id} value={membership.organization_id}>
                  {membership.name}
                </option>
              ))}
            </select>
          </label>
        )}
        <nav className={styles.nav} aria-label={t("shell-nav")}>
          <Link to="/events" className={styles.navLink}>
            {t("nav-events")}
          </Link>
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
        {failure && (
          <p className={styles.failure} role="alert">
            {failure}
          </p>
        )}
      </aside>
      <Fragment key={organization.organization_id}>{children}</Fragment>
    </div>
  );
}
