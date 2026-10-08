import { lazy, Suspense } from "react";
import { t } from "../i18n";
import { Skeleton } from "../ui/Skeleton";

// The Markdown parser is large. It loads with the first notice, so that it stays out of the
// initial JavaScript (ADR 0024).
const Markdown = lazy(() =>
  import("../ui/Markdown").then((module) => ({ default: module.Markdown })),
);

/**
 * The privacy notice of the organization (ADR 0045). The text of the owner replaces the template,
 * which is the one authority of the default text. The page of the member and the invitation page
 * both show it.
 */
export function PrivacyNoticeText({ markdown }: { markdown: string | null | undefined }) {
  return (
    <Suspense fallback={<Skeleton />}>
      <Markdown>{markdown ?? t("privacy-template")}</Markdown>
    </Suspense>
  );
}
