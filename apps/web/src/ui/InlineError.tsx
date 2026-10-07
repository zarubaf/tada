import type { ReactNode } from "react";
import { t } from "../i18n";
import { Button } from "./Button";
import styles from "./InlineError.module.css";

export interface InlineErrorProps {
  message: string;
  /** The ID that an operator needs to find the log lines. */
  requestId?: string | undefined;
  /** Without it, a retry makes no sense, for example after too many requests. */
  onRetry?: () => void;
  /** Another way out, for example a link to the sign-in page. */
  children?: ReactNode;
}

/** A failed request, in the area that failed, with „Erneut versuchen“ (doc/design/components.md). */
export function InlineError({ message, requestId, onRetry, children }: InlineErrorProps) {
  return (
    <div className={styles.error} role="alert">
      <p>{message}</p>
      {requestId && <p className={styles.requestId}>{t("problem-request-id", { requestId })}</p>}
      {onRetry && <Button onPress={onRetry}>{t("retry")}</Button>}
      {children}
    </div>
  );
}
