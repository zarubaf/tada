import { t } from "../i18n";
import { EmptyState } from "./EmptyState";

/**
 * The no-access state of an area of the organization (doc/design/components.md): the refusal and
 * the roles that can give access. A page shows it instead of its content to a member without the role.
 */
export function NoAccess() {
  return <EmptyState title={t("no-access-title")} text={t("no-access-text")} />;
}
