import type { ReactNode } from "react";
import { t } from "../i18n";
import { NavLink, SubNav } from "../ui/NavLink";
import { Page } from "../ui/Page";

/**
 * The settings template (doc/design/layout-and-responsiveness.md): the sub-navigation of the
 * `/settings/*` pages, then the page in `children`. A settings page adds its link here.
 */
export function SettingsLayout({ children }: { children: ReactNode }) {
  return (
    <Page>
      <SubNav label={t("settings-nav")}>
        <NavLink to="/settings/members">{t("settings-nav-members")}</NavLink>
        <NavLink to="/settings/telegram">{t("settings-nav-telegram")}</NavLink>
      </SubNav>
      {children}
    </Page>
  );
}
