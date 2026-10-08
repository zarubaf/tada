// The app shell (doc/design/layout-and-responsiveness.md): a sidebar on medium and wide layouts, a
// top bar and a bottom bar on narrow layouts. It shows only the items that exist.
import { Fragment, type ReactNode, useState } from "react";
import { type Api, problemMessage } from "../api/client";
import { t } from "../i18n";
import { useNavigate, usePathname } from "../router/Router";
import { CHOOSE_ORGANIZATION_PATH, isPublicPath } from "../session/paths";
import { useOptionalSession } from "../session/SessionProvider";
import { LiveRegion } from "../ui/LiveRegion";
import { Menu } from "../ui/Menu";
import { NavLink } from "../ui/NavLink";
import { Select } from "../ui/Select";
import styles from "./Shell.module.css";

export function Shell({ api, children }: { api: Api; children: ReactNode }) {
  const session = useOptionalSession();
  const [failure, setFailure] = useState<string>();
  const pathname = usePathname();
  const navigate = useNavigate();
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
      <header className={styles.side}>
        <p className={styles.brand}>{t("app-name")}</p>
        {memberships.length > 1 && (
          <div className={styles.switcher}>
            <Select
              label={t("organization-switcher")}
              options={memberships.map((membership) => ({
                id: membership.organization_id,
                label: membership.name,
              }))}
              value={organization.organization_id}
              onChange={(id) => void choose(id)}
            />
          </div>
        )}
        <nav className={styles.nav} aria-label={t("shell-nav")}>
          <NavLink to="/events" large>
            {t("nav-events")}
          </NavLink>
          <NavLink to="/settings/members" within="/settings" large>
            {t("nav-settings")}
          </NavLink>
        </nav>
        <div className={styles.member}>
          <footer className={styles.footer}>
            <NavLink to="/privacy">{t("nav-privacy")}</NavLink>
          </footer>
          <Menu
            trigger={user.displayName}
            label={t("member-menu")}
            items={[
              { id: "privacy", label: t("nav-privacy") },
              { id: "sign-out", label: t("sign-out") },
            ]}
            onAction={(id) =>
              id === "privacy" ? navigate("/privacy") : void signOut().then(setFailure)
            }
            placement="top end"
          />
        </div>
        <LiveRegion kind="alert" className={styles.failure}>
          {failure}
        </LiveRegion>
      </header>
      <Fragment key={organization.organization_id}>{children}</Fragment>
    </div>
  );
}
