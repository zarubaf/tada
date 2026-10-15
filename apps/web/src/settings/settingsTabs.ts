import { useCallback, useEffect, useMemo, useState } from "react";
import type { OrganizationRole } from "../api/client";
import { t } from "../i18n";
import { canManage } from "../members/roles";
import { useSearch, useSetSearch } from "../router/Router";

/** The tabs of „Einstellungen“ from left to right (ADR 0082). The first is the default. */
export const SETTINGS_TABS = [
  "account",
  "members",
  "organization",
  "ai-pm",
  "telegram",
  "tokens",
] as const;
export type SettingsTab = (typeof SETTINGS_TABS)[number];

const PARAM = "tab";
const DEFAULT: SettingsTab = "account";

const LABELS: Record<SettingsTab, () => string> = {
  account: () => t("settings-nav-account"),
  members: () => t("settings-nav-members"),
  organization: () => t("settings-nav-organization"),
  "ai-pm": () => t("settings-nav-ai-pm"),
  telegram: () => t("settings-nav-telegram"),
  tokens: () => t("settings-nav-tokens"),
};

export function tabLabel(tab: SettingsTab): string {
  return LABELS[tab]();
}

/** The tabs that the role may open. The server refuses the AI-PM to anyone but a manager. */
export function visibleTabs(role: OrganizationRole | undefined): SettingsTab[] {
  return SETTINGS_TABS.filter((tab) => tab !== "ai-pm" || canManage(role));
}

/**
 * The tab to show for a value of the query. An unknown value or a tab that the role may not see
 * shows the first tab; `denied` tells the second case, so that the page can say why.
 */
export function resolveTab(
  value: string | null,
  role: OrganizationRole | undefined,
): { tab: SettingsTab; denied: boolean } {
  const visible = visibleTabs(role);
  const known = SETTINGS_TABS.find((tab) => tab === value);
  if (known !== undefined && visible.includes(known)) {
    return { tab: known, denied: false };
  }
  return { tab: DEFAULT, denied: known !== undefined };
}

/** Where an old settings path goes: its tab, with the query and the hash kept. */
export function redirectTarget(pathname: string, search: string, hash: string): string {
  const tab = SETTINGS_TABS.find((candidate) => pathname === `/settings/${candidate}`);
  const params = new URLSearchParams(search);
  if (tab !== undefined && tab !== DEFAULT) {
    params.set(PARAM, tab);
  }
  const query = params.toString();
  return `/settings${query ? `?${query}` : ""}${hash}`;
}

/**
 * The tab of the page, kept in the query key `tab`; the first tab has no key. A change makes a new
 * history entry, because a tab is a place to go back to. A tab that the role may not see leaves
 * the address at once and shows as `denied` until the member picks a tab.
 */
export function useSettingsTab(role: OrganizationRole | undefined) {
  const search = useSearch();
  const setSearch = useSetSearch();
  const value = search.get(PARAM);
  const resolved = useMemo(() => resolveTab(value, role), [value, role]);
  const [noted, setNoted] = useState(resolved.denied);

  useEffect(() => {
    if (resolved.denied) {
      setNoted(true);
      setSearch({ [PARAM]: null }, { replace: true });
    }
  }, [resolved.denied, setSearch]);

  const select = useCallback(
    (tab: SettingsTab) => {
      setNoted(false);
      setSearch({ [PARAM]: tab === DEFAULT ? null : tab });
    },
    [setSearch],
  );
  return { tab: resolved.tab, denied: noted || resolved.denied, select };
}
