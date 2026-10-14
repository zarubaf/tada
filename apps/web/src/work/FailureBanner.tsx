import { IconAlertTriangle } from "@tabler/icons-react";
import { t } from "../i18n";
import { Banner } from "../ui/Banner";
import { Button } from "../ui/Button";

/** A banner for data that failed to load, with „Erneut versuchen“. */
export function FailureBanner({
  message,
  detail,
  busy,
  onRetry,
}: {
  message: string;
  detail: string | undefined;
  /** True while the retry runs. */
  busy: boolean;
  onRetry: () => void;
}) {
  return (
    <div aria-busy={busy}>
      <Banner icon={IconAlertTriangle} action={<Button onPress={onRetry}>{t("retry")}</Button>}>
        <p>
          <strong>{message}</strong>
        </p>
        {detail && <p>{detail}</p>}
      </Banner>
    </div>
  );
}
