// The app shell (doc/design/layout-and-responsiveness.md): a sidebar on medium and wide layouts, a
// top bar and a bottom bar on narrow layouts. It shows only the items that exist.
import { IconRefresh } from "@tabler/icons-react";
import { Fragment, type ReactNode, useState } from "react";
import { type Api, problemMessage } from "../api/client";
import { t } from "../i18n";
import { InboxCount } from "../review/InboxCount";
import { useNavigate, usePathname } from "../router/Router";
import { CHOOSE_ORGANIZATION_PATH, isPublicPath } from "../session/paths";
import { useOptionalSession } from "../session/SessionProvider";
import { Banner } from "../ui/Banner";
import { Button } from "../ui/Button";
import { LiveRegion } from "../ui/LiveRegion";
import { Menu } from "../ui/Menu";
import { NavLink } from "../ui/NavLink";
import { Select } from "../ui/Select";
import styles from "./Shell.module.css";
import { VersionButton, VersionSheet } from "./VersionFooter";
import { footerText, outdated as isOutdated, useServerVersion } from "./version";

export function Shell({ api, children }: { api: Api; children: ReactNode }) {
  const session = useOptionalSession();
  const [failure, setFailure] = useState<string>();
  const pathname = usePathname();
  const navigate = useNavigate();
  const frameless = isPublicPath(pathname) || pathname === CHOOSE_ORGANIZATION_PATH;
  const [versionsOpen, setVersionsOpen] = useState(false);
  const server = useServerVersion(api, !frameless && session?.organization !== undefined);
  const outdated = server !== undefined && isOutdated(server);

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
          <NavLink to="/" exact large>
            {t("nav-my-work")}
          </NavLink>
          <NavLink to="/inbox" large>
            {t("nav-inbox")}
            <InboxCount />
          </NavLink>
          <NavLink to="/events" large>
            {t("nav-events")}
          </NavLink>
          {/* The bottom bar has room for four items (My Work, Inbox, Events, Settings); on narrow layouts the member menu has these. */}
          <div className={styles.registers}>
            <NavLink to="/persons">{t("nav-persons")}</NavLink>
            <NavLink to="/institutions">{t("nav-institutions")}</NavLink>
          </div>
          <NavLink to="/settings/members" within="/settings" large>
            {t("nav-settings")}
          </NavLink>
        </nav>
        <div className={styles.member}>
          <footer className={styles.footer}>
            <NavLink to="/privacy">{t("nav-privacy")}</NavLink>
            <VersionButton server={server} onPress={() => setVersionsOpen(true)} />
          </footer>
          <Menu
            trigger={user.displayName}
            label={t("member-menu")}
            items={[
              { id: "persons", label: t("nav-persons") },
              { id: "institutions", label: t("nav-institutions") },
              { id: "privacy", label: t("nav-privacy") },
              {
                id: "versions",
                label: server
                  ? t("member-menu-versions", { build: footerText(server) })
                  : t("versions-title"),
              },
              { id: "sign-out", label: t("sign-out") },
            ]}
            onAction={(id) => {
              if (id === "sign-out") {
                void signOut().then(setFailure);
              } else if (id === "versions") {
                setVersionsOpen(true);
              } else {
                navigate(`/${id}`);
              }
            }}
            placement="top end"
          />
        </div>
        {/* The prompt to load again appears without an action of the member: this region announces it. */}
        <LiveRegion kind="status" visuallyHidden>
          {outdated ? `${t("version-outdated-title")} ${t("version-outdated-text")}` : undefined}
        </LiveRegion>
        <LiveRegion kind="alert" className={styles.failure}>
          {failure}
        </LiveRegion>
      </header>
      <VersionSheet api={api} isOpen={versionsOpen} onClose={() => setVersionsOpen(false)} />
      {outdated && (
        <div className={styles.outdated}>
          <Banner
            icon={IconRefresh}
            action={<Button onPress={() => window.location.reload()}>{t("version-reload")}</Button>}
          >
            <p>
              <strong>{t("version-outdated-title")}</strong>
            </p>
            <p>{t("version-outdated-text")}</p>
          </Banner>
        </div>
      )}
      <Fragment key={organization.organization_id}>{children}</Fragment>
    </div>
  );
}
