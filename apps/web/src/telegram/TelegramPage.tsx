import { useCallback, useEffect, useRef, useState } from "react";
import {
  type Api,
  problemMessage,
  type TelegramLinkCode,
  type TelegramLinkRequest,
} from "../api/client";
import { failureOf } from "../api/failure";
import { LOCALE, t } from "../i18n";
import { Button } from "../ui/Button";
import { ConfirmDialog } from "../ui/ConfirmDialog";
import { DataTable } from "../ui/DataTable";
import { EmptyState } from "../ui/EmptyState";
import { useFocusAfterCommit } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { LiveRegion } from "../ui/LiveRegion";
import { PageTitle } from "../ui/Page";
import { Skeleton } from "../ui/Skeleton";
import styles from "./TelegramPage.module.css";

const claimedFormat = new Intl.DateTimeFormat(LOCALE, { dateStyle: "medium", timeStyle: "short" });
const expiryFormat = new Intl.DateTimeFormat(LOCALE, { timeStyle: "short" });

type Requests =
  | { kind: "loading"; attempts: number }
  | { kind: "failed"; message: string; requestId: string | undefined; attempts: number }
  | { kind: "loaded"; items: TelegramLinkRequest[]; attempts: number };

/**
 * „Telegram verknüpfen“ in the settings (ADR 0011): the member creates a link code, sends it to
 * the bot and confirms the claim of the Telegram account. The code shows once, right after its
 * creation, and the page never stores it.
 */
export function TelegramPage({ api }: { api: Api }) {
  const [requests, setRequests] = useState<Requests>({ kind: "loading", attempts: 0 });
  const [code, setCode] = useState<TelegramLinkCode>();
  const [confirming, setConfirming] = useState<TelegramLinkRequest>();
  // A request runs: a second press does nothing.
  const [busy, setBusy] = useState(false);
  // Both live regions are in the page from the start, so that a text set later is announced.
  const [failure, setFailure] = useState<string>();
  const [confirmation, setConfirmation] = useState<string>();
  const alert = useRef<HTMLParagraphElement>(null);
  const codeBox = useRef<HTMLDivElement>(null);
  const requestsHeading = useRef<HTMLHeadingElement>(null);
  const focusAfterCommit = useFocusAfterCommit();

  const load = useCallback(
    async (attempts: number) => {
      try {
        const { data, error } = await api.GET("/api/v1/telegram/link-requests");
        if (data && attempts > 0) {
          setConfirmation(t("telegram-requests-refreshed"));
        }
        setRequests(
          data
            ? { kind: "loaded", items: data.items, attempts }
            : {
                kind: "failed",
                message: problemMessage(error),
                requestId: error?.request_id,
                attempts,
              },
        );
      } catch {
        setRequests({
          kind: "failed",
          message: problemMessage(undefined),
          requestId: undefined,
          attempts,
        });
      }
    },
    [api],
  );

  useEffect(() => {
    void load(0);
  }, [load]);

  const showFailure = (message: string) => {
    setFailure(message);
    // The failure may be far above the button that the member pressed.
    alert.current?.scrollIntoView?.({ block: "nearest" });
  };

  const createCode = async () => {
    if (busy) {
      return;
    }
    setFailure(undefined);
    setConfirmation(undefined);
    setBusy(true);
    try {
      const result = await api.POST("/api/v1/telegram/link-codes");
      if (result.data) {
        setCode(result.data);
        // Focus goes to the code, so that a screen reader reads it.
        focusAfterCommit(() => codeBox.current);
      } else {
        showFailure(failureOf(result).message);
      }
    } catch {
      showFailure(failureOf({}).message);
    }
    setBusy(false);
  };

  const confirm = async () => {
    if (busy || !confirming) {
      return;
    }
    const item = confirming;
    setFailure(undefined);
    setConfirmation(undefined);
    setBusy(true);
    try {
      const result = await api.POST("/api/v1/telegram/link-requests/{request_id}/confirm", {
        params: { path: { request_id: item.id } },
      });
      if (result.response.ok) {
        // The pressed button leaves with its row: focus goes to the heading of the list.
        focusAfterCommit(() => requestsHeading.current);
        setConfirmation(t("telegram-linked"));
        // The code has done its work, and a stale code would mislead.
        setCode(undefined);
        setRequests((current) =>
          current.kind === "loaded"
            ? { ...current, items: current.items.filter((r) => r.id !== item.id) }
            : current,
        );
      } else {
        showFailure(failureOf(result).message);
        if (result.error?.code === "not-found") {
          // The request is gone or taken: the list shows the truth again.
          focusAfterCommit(() => requestsHeading.current);
          void load(requests.attempts);
        }
      }
    } catch {
      showFailure(failureOf({}).message);
    }
    setBusy(false);
    setConfirming(undefined);
  };

  const refresh = () => {
    const attempts = requests.attempts + 1;
    setFailure(undefined);
    setConfirmation(undefined);
    setRequests({ kind: "loading", attempts });
    void load(attempts);
  };

  return (
    <div className={styles.page}>
      <LiveRegion ref={alert} kind="alert">
        {failure}
      </LiveRegion>
      <LiveRegion kind="status">{confirmation}</LiveRegion>

      <section className={styles.section} aria-labelledby="telegram-title">
        <PageTitle id="telegram-title">{t("telegram-title")}</PageTitle>
        <p>{t("telegram-intro")}</p>
        <h2 className={styles.heading}>{t("telegram-steps-title")}</h2>
        <ol className={styles.steps}>
          <li>{t("telegram-step-create")}</li>
          <li>{t("telegram-step-send")}</li>
          <li>{t("telegram-step-confirm")}</li>
        </ol>
        <p className={styles.check}>{t("telegram-check")}</p>
        <div>
          <Button
            variant="primary"
            isPending={busy && !confirming}
            onPress={() => void createCode()}
          >
            {t("telegram-code-create")}
          </Button>
        </div>
        {code && (
          <div ref={codeBox} tabIndex={-1} className={styles.code}>
            <p className={styles.codeLabel}>{t("telegram-code-label")}</p>
            <p className={styles.codeValue}>{code.code}</p>
            <p>
              {t("telegram-code-expires", { time: expiryFormat.format(new Date(code.expires_at)) })}
            </p>
          </div>
        )}
      </section>

      <section className={styles.section} aria-labelledby="telegram-requests-title">
        <h2
          id="telegram-requests-title"
          ref={requestsHeading}
          tabIndex={-1}
          className={styles.heading}
        >
          {t("telegram-requests-title")}
        </h2>
        {requests.kind === "loading" && (
          <div
            className={styles.skeleton}
            role="status"
            aria-label={t("telegram-requests-loading")}
          >
            <Skeleton />
            <Skeleton />
          </div>
        )}
        {requests.kind === "failed" && (
          <InlineError
            message={requests.message}
            requestId={requests.requestId}
            onRetry={refresh}
            takeFocus={requests.attempts > 0}
          />
        )}
        {requests.kind === "loaded" && requests.items.length === 0 && (
          <EmptyState
            title={t("telegram-requests-empty-title")}
            text={t("telegram-requests-empty-text")}
          />
        )}
        {requests.kind === "loaded" && requests.items.length > 0 && (
          <DataTable
            label={t("telegram-requests-title")}
            columns={[
              {
                id: "name",
                header: t("telegram-column-name"),
                cell: (request) => request.telegram_name,
              },
              {
                id: "telegram-id",
                header: t("telegram-column-id"),
                cell: (request) => String(request.telegram_user_id),
                mono: true,
              },
              {
                id: "claimed",
                header: t("telegram-column-claimed"),
                cell: (request) => (
                  <time dateTime={request.claimed_at}>
                    {claimedFormat.format(new Date(request.claimed_at))}
                  </time>
                ),
                numeric: true,
              },
              {
                id: "actions",
                header: t("telegram-column-actions"),
                cell: (request) => (
                  <Button
                    aria-label={t("telegram-confirm-of", { name: request.telegram_name })}
                    onPress={() => setConfirming(request)}
                  >
                    {t("telegram-confirm")}
                  </Button>
                ),
              },
            ]}
            rows={requests.items}
            rowKey={(request) => request.id}
          />
        )}
        {/* The button stays in the page while the list loads, so that it keeps focus. */}
        <div>
          <Button isPending={requests.kind === "loading"} onPress={refresh}>
            {t("telegram-requests-refresh")}
          </Button>
        </div>
      </section>

      <ConfirmDialog
        isOpen={confirming !== undefined}
        title={t("telegram-confirm-title")}
        text={t("telegram-confirm-text", {
          name: confirming?.telegram_name ?? "",
          id: String(confirming?.telegram_user_id ?? ""),
          time: confirming ? claimedFormat.format(new Date(confirming.claimed_at)) : "",
        })}
        confirmLabel={t("telegram-confirm-submit")}
        cancelLabel={t("telegram-confirm-cancel")}
        isPending={busy}
        onConfirm={() => void confirm()}
        onCancel={() => !busy && setConfirming(undefined)}
      />
    </div>
  );
}
