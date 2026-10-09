import { t } from "./i18n";
import { Page, PageTitle } from "./ui/Page";

/** The page for a path that no route matches. */
export function NotFoundPage() {
  return (
    <Page>
      <PageTitle>{t("not-found-title")}</PageTitle>
      <p>{t("not-found-text")}</p>
    </Page>
  );
}
