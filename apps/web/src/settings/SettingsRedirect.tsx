import { NotFoundPage } from "../NotFoundPage";
import { Redirect, useParams } from "../router/Router";
import { redirectTarget, SETTINGS_TABS } from "./settingsTabs";

/**
 * An old settings address, for example `/settings/telegram`, goes to its tab and keeps the query
 * and the hash. The redirect replaces the entry, so that Back does not return to the old address.
 */
export function SettingsRedirect() {
  const { tab } = useParams();
  const known = SETTINGS_TABS.some((candidate) => candidate === tab);
  if (!known) {
    return <NotFoundPage />;
  }
  const { pathname, search, hash } = window.location;
  return <Redirect to={redirectTarget(pathname, search, hash)} />;
}
