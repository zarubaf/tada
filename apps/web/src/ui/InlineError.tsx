import { t } from "../i18n";
import { Button } from "./Button";
import styles from "./InlineError.module.css";

export interface InlineErrorProps {
  message: string;
  /** The ID that an operator needs to find the log lines. */
  requestId?: string | undefined;
  onRetry: () => void;
}

/** A failed request, in the area that failed, with „Erneut versuchen“ (doc/design/components.md). */
export function InlineError({ message, requestId, onRetry }: InlineErrorProps) {
  return (
    <div className={styles.error} role="alert">
      <p>{message}</p>
      {requestId && <p className={styles.requestId}>{t("problem-request-id", { requestId })}</p>}
      <Button onPress={onRetry}>{t("retry")}</Button>
    </div>
  );
}
