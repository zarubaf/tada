import type { Author } from "../api/client";
import { authorKindName } from "../evidence/authorKind";
import { t } from "../i18n";

/** Who proposed a change, by the kind of the author. */
export function authorName(author: Author): string {
  return author.kind === "member" ? t("inbox-author-member") : authorKindName(author.kind);
}
