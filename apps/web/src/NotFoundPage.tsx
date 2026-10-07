import { t } from "./i18n";
import { EmptyState } from "./ui/EmptyState";

/** The page for a path that no route matches. */
export function NotFoundPage() {
  return (
    <main id="main">
      <EmptyState title={t("not-found-title")} text={t("not-found-text")} />
    </main>
  );
}
