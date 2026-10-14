// The app shell (doc/design/layout-and-responsiveness.md): a sidebar, a top bar on all widths, and on
// narrow layouts a bottom bar. It shows only the items that exist.
import { IconRefresh } from "@tabler/icons-react";
import { Fragment, type ReactNode, useState } from "react";
import { type Api, problemMessage } from "../api/client";
import { t } from "../i18n";
import { useNavigate, usePathname } from "../router/Router";
import { CHOOSE_ORGANIZATION_PATH, isPublicPath } from "../session/paths";
import { useOptionalSession } from "../session/SessionProvider";
import { Banner } from "../ui/Banner";
import { Button } from "../ui/Button";
import { LiveRegion } from "../ui/LiveRegion";
import { Select } from "../ui/Select";
import { sectionTitle } from "./navItems";
import { readRemembered, SIDEBAR_KEY, writeRemembered } from "./remembered";
import styles from "./Shell.module.css";
import { Sidebar } from "./Sidebar";
import { TopBar } from "./TopBar";
import { VersionSheet } from "./VersionFooter";
import { footerText, outdated as isOutdated, useServerVersion } from "./version";

export function Shell({ api, children }: { api: Api; children: ReactNode }) {
  const session = useOptionalSession();
  const [failure, setFailure] = useState<string>();
  const pathname = usePathname();
  const navigate = useNavigate();
  const frameless = isPublicPath(pathname) || pathname === CHOOSE_ORGANIZATION_PATH;
  const [versionsOpen, setVersionsOpen] = useState(false);
  const [sidebarOpen, setSidebarOpen] = useState(() => readRemembered(SIDEBAR_KEY) !== "collapsed");
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

  const toggleSidebar = () => {
    writeRemembered(SIDEBAR_KEY, sidebarOpen ? "collapsed" : undefined);
    setSidebarOpen(!sidebarOpen);
  };

  const memberItems = [
    { id: "settings/account", label: t("settings-nav-account") },
    { id: "privacy", label: t("nav-privacy") },
    {
      id: "versions",
      label: server
        ? t("member-menu-versions", { build: footerText(server) })
        : t("versions-title"),
    },
    { id: "sign-out", label: t("sign-out") },
  ];

  const organizationSwitcher = memberships.length > 1 && (
    <Select
      label={t("organization-switcher")}
      options={memberships.map((membership) => ({
        id: membership.organization_id,
        label: membership.name,
      }))}
      value={organization.organization_id}
      onChange={(id) => void choose(id)}
    />
  );

  return (
    <div className={styles.shell} data-sidebar={sidebarOpen ? "open" : "closed"}>
      {sidebarOpen && (
        <div className={styles.sidebar}>
          <Sidebar
            organizationSwitcher={organizationSwitcher}
            eventSwitcher={null}
            server={server}
            onOpenVersions={() => setVersionsOpen(true)}
            member={{
              id: user.id,
              name: user.displayName,
              role: t(`role-${organization.role}`),
            }}
          />
        </div>
      )}
      <div className={styles.workspace}>
        <TopBar
          title={sectionTitle(pathname) ?? t("app-name")}
          toggleLabel={sidebarOpen ? t("shell-sidebar-close") : t("shell-sidebar-open")}
          expanded={sidebarOpen}
          onToggle={toggleSidebar}
          onOpenPalette={() => undefined}
          member={{ id: user.id, name: user.displayName }}
          memberItems={memberItems}
          onMemberAction={(id) => {
            if (id === "sign-out") {
              void signOut().then(setFailure);
            } else if (id === "versions") {
              setVersionsOpen(true);
            } else {
              navigate(`/${id}`);
            }
          }}
        />
        {/* The prompt to load again appears without an action of the member: this region announces it. */}
        <LiveRegion kind="status" visuallyHidden>
          {outdated ? `${t("version-outdated-title")} ${t("version-outdated-text")}` : undefined}
        </LiveRegion>
        <LiveRegion kind="alert" className={styles.failure}>
          {failure}
        </LiveRegion>
        {outdated && (
          <div className={styles.outdated}>
            <Banner
              icon={IconRefresh}
              action={
                <Button onPress={() => window.location.reload()}>{t("version-reload")}</Button>
              }
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
      <VersionSheet api={api} isOpen={versionsOpen} onClose={() => setVersionsOpen(false)} />
    </div>
  );
}
