// The app shell (doc/design/layout-and-responsiveness.md): a sidebar, a top bar on all widths, and on
// narrow layouts a bottom bar. It shows only the items that exist.
import { IconRefresh } from "@tabler/icons-react";
import { Fragment, lazy, type ReactNode, Suspense, useEffect, useState } from "react";
import { type Api, problemMessage } from "../api/client";
import { useCanCreateEvent } from "../events/useCanCreateEvent";
import { t } from "../i18n";
import { useNavigate, usePathname } from "../router/Router";
import { CHOOSE_ORGANIZATION_PATH, isPublicPath } from "../session/paths";
import { useOptionalSession } from "../session/SessionProvider";
import { Banner } from "../ui/Banner";
import { Button } from "../ui/Button";
import { LiveRegion } from "../ui/LiveRegion";
import { Select } from "../ui/Select";
import { BottomBar } from "./BottomBar";
import { ALL_EVENTS, EventSwitcher, eventIdOfItem, NEW_EVENT } from "./EventSwitcher";
import { eventIdOf, sectionTitle } from "./navItems";
import { readRemembered, SIDEBAR_KEY, writeRemembered } from "./remembered";
import styles from "./Shell.module.css";
import { Sidebar } from "./Sidebar";
import { isPaletteKey } from "./shortcut";
import { TopBar } from "./TopBar";
import { useCurrentEvent, useEventList } from "./useEvents";
import { useNarrow } from "./useNarrow";
import { VersionSheet } from "./VersionFooter";
import { footerText, outdated as isOutdated, useServerVersion } from "./version";

// The drawer is only for narrow layouts: its code loads with the first opening (ADR 0024).
const Drawer = lazy(() => import("../ui/Drawer"));
const CommandPalette = lazy(() => import("./CommandPalette"));

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
  const organizationId = session?.organization?.organization_id;
  const routeEventId = eventIdOf(pathname);
  // A page without the frame has no organization to ask for events.
  const framed = session?.organization !== undefined && !frameless;
  const framedOrganizationId = framed ? organizationId : undefined;
  const currentEvent = useCurrentEvent(api, routeEventId, framedOrganizationId);
  const events = useEventList(api, framedOrganizationId);
  const canCreateEvent = useCanCreateEvent();
  const narrow = useNarrow();
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);

  // Ctrl+K and ⌘K open the palette from every place of the framed shell.
  useEffect(() => {
    if (!framed) {
      return;
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (isPaletteKey(event)) {
        event.preventDefault();
        setPaletteOpen(true);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [framed]);

  // A destination or an event that opens ends the drawer.
  // biome-ignore lint/correctness/useExhaustiveDependencies: the path change is the trigger
  useEffect(() => setDrawerOpen(false), [pathname]);

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

  const openEvent = (item: string) => {
    const eventId = eventIdOfItem(item);
    if (eventId !== undefined) {
      navigate(`/events/${encodeURIComponent(eventId)}`);
    } else if (item === ALL_EVENTS) {
      navigate("/events");
    } else if (item === NEW_EVENT) {
      navigate("/events/new");
    }
  };

  // On narrow layouts the toggle opens the drawer; the remembered state belongs to the wide layouts.
  const toggleSidebar = () => {
    if (narrow) {
      setDrawerOpen(true);
      return;
    }
    writeRemembered(SIDEBAR_KEY, sidebarOpen ? "collapsed" : undefined);
    setSidebarOpen(!sidebarOpen);
  };

  const openVersions = () => {
    setDrawerOpen(false);
    setVersionsOpen(true);
  };

  const versionsLabel = server
    ? t("member-menu-versions", { build: footerText(server) })
    : t("versions-title");

  const memberItems = [
    { id: "settings/account", label: t("settings-nav-account") },
    { id: "privacy", label: t("nav-privacy") },
    { id: "versions", label: versionsLabel },
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

  const sidebar = (
    <Sidebar
      organizationSwitcher={organizationSwitcher}
      eventSwitcher={
        <EventSwitcher
          current={currentEvent}
          list={events.list}
          canCreate={canCreateEvent}
          onOpen={events.load}
          onAction={openEvent}
        />
      }
      server={server}
      onOpenVersions={openVersions}
      member={{ id: user.id, name: user.displayName, role: t(`role-${organization.role}`) }}
    />
  );

  return (
    <div className={styles.shell} data-sidebar={sidebarOpen && !narrow ? "open" : "closed"}>
      {sidebarOpen && !narrow && <div className={styles.sidebar}>{sidebar}</div>}
      <div className={styles.workspace}>
        <TopBar
          title={
            (routeEventId !== undefined ? currentEvent?.name : undefined) ??
            sectionTitle(pathname) ??
            t("app-name")
          }
          toggleLabel={
            narrow
              ? t("shell-drawer-open")
              : sidebarOpen
                ? t("shell-sidebar-close")
                : t("shell-sidebar-open")
          }
          expanded={narrow ? false : sidebarOpen}
          onToggle={toggleSidebar}
          onOpenPalette={() => setPaletteOpen(true)}
          member={{ id: user.id, name: user.displayName }}
          memberItems={memberItems}
          onMemberAction={(id) => {
            if (id === "sign-out") {
              void signOut().then(setFailure);
            } else if (id === "versions") {
              openVersions();
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
      {narrow && <BottomBar versionsLabel={versionsLabel} onOpenVersions={openVersions} />}
      {narrow && drawerOpen && (
        <Suspense fallback={null}>
          <Drawer title={t("shell-drawer-title")} onClose={() => setDrawerOpen(false)}>
            {sidebar}
          </Drawer>
        </Suspense>
      )}
      {paletteOpen && (
        <Suspense fallback={null}>
          <CommandPalette
            events={events.list}
            loadEvents={events.load}
            currentEventId={currentEvent?.id}
            canCreateEvent={canCreateEvent}
            onClose={() => setPaletteOpen(false)}
          />
        </Suspense>
      )}
      <VersionSheet api={api} isOpen={versionsOpen} onClose={() => setVersionsOpen(false)} />
    </div>
  );
}
