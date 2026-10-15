import { IconLock } from "@tabler/icons-react";
import { type ComponentType, lazy } from "react";
import type { Api } from "../api/client";
import { t } from "../i18n";
import { useOptionalSession } from "../session/SessionProvider";
import { useNarrow } from "../shell/useNarrow";
import { Banner } from "../ui/Banner";
import { Page, PageTitle } from "../ui/Page";
import { PageToolbar } from "../ui/PageToolbar";
import { RouteSuspense } from "../ui/RouteSuspense";
import { Select } from "../ui/Select";
import { TabPanel, Tabs } from "../ui/Tabs";
import styles from "./SettingsPage.module.css";
import { type SettingsTab, tabLabel, useSettingsTab, visibleTabs } from "./settingsTabs";

// Each tab loads on demand: a member opens one of them at a time, and the first load stays inside
// the bundle budget of ADR 0024.
const PANELS: Record<SettingsTab, ComponentType<{ api: Api }>> = {
  account: lazy(() => import("./AccountPage").then((module) => ({ default: module.AccountPage }))),
  members: lazy(() =>
    import("../members/MembersPage").then((module) => ({ default: module.MembersPage })),
  ),
  organization: lazy(() =>
    import("./OrganizationPage").then((module) => ({ default: module.OrganizationPage })),
  ),
  "ai-pm": lazy(() => import("./PmPage").then((module) => ({ default: module.PmPage }))),
  telegram: lazy(() =>
    import("../telegram/TelegramPage").then((module) => ({ default: module.TelegramPage })),
  ),
  tokens: lazy(() =>
    import("../tokens/TokensPage").then((module) => ({ default: module.TokensPage })),
  ),
};

/**
 * „Einstellungen“ (ADR 0082): one page with a tab for each area. The page owns the `h1` and the
 * tab row in one toolbar row, as the „Arbeit“ page does; the panel holds the page of the tab. A
 * phone has a select in place of the tabs, because six tabs do not fit. A tab that the role may not
 * see is not in the list; its address shows the first tab and says why.
 */
export function SettingsPage({ api }: { api: Api }) {
  const role = useOptionalSession()?.organization?.role;
  const { tab, denied, select } = useSettingsTab(role);
  const narrow = useNarrow();
  const options = visibleTabs(role).map((id) => ({ id, label: tabLabel(id) }));
  const Panel = PANELS[tab];
  const title = <PageTitle compact>{t("nav-settings")}</PageTitle>;
  const content = (
    <div className={styles.panel}>
      {denied && (
        <Banner icon={IconLock}>
          <strong>{t("no-access-title")}</strong> {t("no-access-text")}
        </Banner>
      )}
      <RouteSuspense bare label={t("settings-loading")}>
        <Panel api={api} />
      </RouteSuspense>
    </div>
  );
  const choose = (id: string) => {
    const next = options.find((option) => option.id === id);
    if (next) {
      select(next.id);
    }
  };
  return (
    <Page width="flush">
      {narrow ? (
        <>
          <PageToolbar>
            {title}
            <div className={styles.select}>
              <Select
                label={t("settings-tabs-label")}
                labelHidden
                variant="toolbar"
                options={options}
                value={tab}
                onChange={choose}
              />
            </div>
          </PageToolbar>
          {content}
        </>
      ) : (
        <Tabs
          label={t("settings-tabs-label")}
          tabs={options}
          value={tab}
          variant="segmented"
          renderList={(list) => (
            <PageToolbar>
              {title}
              {list}
            </PageToolbar>
          )}
          onChange={choose}
        >
          <TabPanel id={tab}>{content}</TabPanel>
        </Tabs>
      )}
    </Page>
  );
}
