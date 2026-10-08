import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";
import { type Api, type PrivacyNotice, problemMessage } from "../api/client";
import { failureOf } from "../api/failure";
import { hasMessage, t } from "../i18n";
import { Button } from "../ui/Button";
import { ConfirmDialog } from "../ui/ConfirmDialog";
import { firstInvalidField, useFocusAfterCommit, useRetry } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { LinkButton } from "../ui/LinkButton";
import { Skeleton } from "../ui/Skeleton";
import { TextField } from "../ui/TextField";
import styles from "./PrivacyNoticeSettings.module.css";

type State = { kind: "loading" } | { kind: "failed" } | { kind: "loaded"; notice: PrivacyNotice };

export interface PrivacyNoticeSettingsProps {
  api: Api;
  /** The server decides; a member sees the link only, so that no action fails. */
  isOwner: boolean;
  /** The page owns the live regions: a result goes to its status region. */
  onStatus: (message: string | undefined) => void;
  /** A failure goes to the alert region of the page. */
  onFailure: (message: string | undefined) => void;
}

/**
 * The privacy notice on the page of the organization (ADR 0045). An owner edits the Markdown text,
 * which starts as the template. A member reads the notice on its own page.
 */
export function PrivacyNoticeSettings({
  api,
  isOwner,
  onStatus,
  onFailure,
}: PrivacyNoticeSettingsProps) {
  return (
    <section className={styles.section} aria-labelledby="org-privacy-title">
      <h2 id="org-privacy-title" tabIndex={-1} className={styles.title}>
        {t("org-privacy-title")}
      </h2>
      {isOwner ? (
        <OwnerForm api={api} onStatus={onStatus} onFailure={onFailure} />
      ) : (
        <>
          <p className={styles.help}>{t("org-privacy-owner-only")}</p>
          <div className={styles.actions}>
            <LinkButton to="/privacy">{t("org-privacy-read")}</LinkButton>
          </div>
        </>
      )}
    </section>
  );
}

function OwnerForm({ api, onStatus, onFailure }: Omit<PrivacyNoticeSettingsProps, "isOwner">) {
  const [state, setState] = useState<State>({ kind: "loading" });
  const [text, setText] = useState("");
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [resetting, setResetting] = useState(false);
  const form = useRef<HTMLFormElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  const { retried, retry } = useRetry(() => document.getElementById("org-privacy-title"));

  const show = useCallback((notice: PrivacyNotice) => {
    setState({ kind: "loaded", notice });
    setText(notice.markdown ?? t("privacy-template"));
  }, []);

  /** Resolves to true when the notice loaded. */
  const load = useCallback(async () => {
    try {
      const { data } = await api.GET("/api/v1/organization/privacy-notice");
      if (data) {
        show(data);
        return true;
      }
    } catch {
      // The same failure state as a problem response.
    }
    setState({ kind: "failed" });
    return false;
  }, [api, show]);

  useEffect(() => {
    void load();
  }, [load]);

  /** Sends the new text (`null` for the template). Resolves to the saved notice, if it worked. */
  const send = async (notice: PrivacyNotice, markdown: string | null) => {
    onFailure(undefined);
    onStatus(undefined);
    setError(undefined);
    setBusy(true);
    let saved: PrivacyNotice | undefined;
    try {
      const result = await api.POST("/api/v1/organization/privacy-notice/set", {
        body: { markdown, expected_version: notice.version },
      });
      if (result.data) {
        saved = result.data;
      } else if (result.error?.code === "validation-failed") {
        const code = result.error.errors?.find((e) => e.pointer === "/markdown")?.code;
        setError(
          code && hasMessage(`org-privacy-error-${code}`)
            ? t(`org-privacy-error-${code}`)
            : t("problem-validation-failed"),
        );
        focusAfterCommit(() => firstInvalidField(form.current));
      } else if (result.error?.code === "record-version-conflict") {
        onFailure(t("org-privacy-conflict"));
      } else {
        onFailure(failureOf(result).message);
      }
    } catch {
      onFailure(problemMessage(undefined));
    }
    setBusy(false);
    return saved;
  };

  const save = async (event: FormEvent) => {
    event.preventDefault();
    if (busy || state.kind !== "loaded") {
      return;
    }
    const saved = await send(state.notice, text);
    if (saved) {
      setState({ kind: "loaded", notice: saved });
      onStatus(t("org-privacy-saved"));
    }
  };

  const reset = async () => {
    if (busy || state.kind !== "loaded") {
      return;
    }
    const saved = await send(state.notice, null);
    setResetting(false);
    if (saved) {
      show(saved);
      onStatus(t("org-privacy-reset-done"));
      // „Vorlage wiederherstellen“ leaves with the own text: focus goes to the field.
      focusAfterCommit(() => form.current?.querySelector<HTMLElement>("textarea"));
    }
  };

  if (state.kind === "loading") {
    return (
      <div className={styles.skeleton} role="status" aria-label={t("org-privacy-loading")}>
        <Skeleton />
      </div>
    );
  }
  if (state.kind === "failed") {
    return (
      <InlineError
        message={problemMessage(undefined)}
        onRetry={() =>
          retry(() => {
            setState({ kind: "loading" });
            return load();
          })
        }
        announce={retried ? "focus" : "alert"}
      />
    );
  }

  const isTemplate = state.notice.markdown === null;
  return (
    <>
      {isTemplate && <p className={styles.help}>{t("org-privacy-template")}</p>}
      <form ref={form} className={styles.form} onSubmit={(event) => void save(event)} noValidate>
        <TextField
          label={t("org-privacy-label")}
          help={t("org-privacy-help")}
          value={text}
          onChange={(value) => {
            setText(value);
            setError(undefined);
          }}
          error={error}
          rows={16}
          isRequired
        />
        <div className={styles.actions}>
          <Button type="submit" variant="primary" isPending={busy}>
            {t("org-privacy-save")}
          </Button>
          {!isTemplate && (
            <Button isPending={busy} onPress={() => setResetting(true)}>
              {t("org-privacy-reset")}
            </Button>
          )}
          <LinkButton to="/privacy">{t("org-privacy-read")}</LinkButton>
        </div>
      </form>
      <ConfirmDialog
        isOpen={resetting}
        title={t("org-privacy-reset-title")}
        text={t("org-privacy-reset-text")}
        confirmLabel={t("org-privacy-reset-confirm")}
        cancelLabel={t("org-privacy-reset-cancel")}
        isPending={busy}
        onConfirm={() => void reset()}
        onCancel={() => !busy && setResetting(false)}
      />
    </>
  );
}
