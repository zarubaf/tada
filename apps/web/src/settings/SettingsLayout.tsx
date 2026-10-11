import type { ReactNode } from "react";
import { t } from "../i18n";
import { canManage } from "../members/roles";
import { useOptionalSession } from "../session/SessionProvider";
import { NavLink, SubNav } from "../ui/NavLink";
import { Page } from "../ui/Page";

/**
 * The settings template (doc/design/layout-and-responsiveness.md): the sub-navigation of the
 * `/settings/*` pages, then the page in `children`. A settings page adds its link here.
 * „AI-PM“ shows only for an owner or an admin; the server refuses the page to anyone else.
 */
export function SettingsLayout({ children }: { children: ReactNode }) {
  const role = useOptionalSession()?.organization?.role;
  return (
    <Page>
      <SubNav label={t("settings-nav")}>
        <NavLink to="/settings/members">{t("settings-nav-members")}</NavLink>
        <NavLink to="/settings/telegram">{t("settings-nav-telegram")}</NavLink>
        <NavLink to="/settings/tokens">{t("settings-nav-tokens")}</NavLink>
        <NavLink to="/settings/organization">{t("settings-nav-organization")}</NavLink>
        {canManage(role) && <NavLink to="/settings/ai-pm">{t("settings-nav-ai-pm")}</NavLink>}
        <NavLink to="/settings/account">{t("settings-nav-account")}</NavLink>
      </SubNav>
      {children}
    </Page>
  );
}
