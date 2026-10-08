import { t } from "../i18n";
import { LazyMarkdown } from "../ui/LazyMarkdown";

/**
 * The privacy notice of the organization (ADR 0045). The text of the owner replaces the template,
 * which is the one authority of the default text. The page of the member and the invitation page
 * both show it. `onShown` reports that the text is in the page.
 */
export function PrivacyNoticeText({
  markdown,
  onShown,
}: {
  markdown: string | null | undefined;
  onShown?: () => void;
}) {
  return (
    <LazyMarkdown {...(onShown ? { onShown } : {})}>
      {markdown ?? t("privacy-template")}
    </LazyMarkdown>
  );
}
