import { hasMessage, t } from "../i18n";

/**
 * The label of an author kind that is not a member. The list of kinds is open (the API says so):
 * a kind that this client does not know gets a generic label instead of a missing message.
 */
export function authorKindName(kind: string): string {
  const id = `evidence-author-${kind}`;
  return hasMessage(id) ? t(id) : t("evidence-author-other");
}
