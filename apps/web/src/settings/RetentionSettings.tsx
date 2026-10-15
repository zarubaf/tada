import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";
import { type Api, type OrganizationRole, problemMessage, type Retention } from "../api/client";
import { failureOf } from "../api/failure";
import { hasMessage, t } from "../i18n";
import { canManage } from "../members/roles";
import { SignInAgain } from "../session/SignInAgain";
import { Button } from "../ui/Button";
import { firstInvalidField, useFocusAfterCommit, useRetry } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { Skeleton } from "../ui/Skeleton";
import { TextField } from "../ui/TextField";
import styles from "./RetentionSettings.module.css";

type State = { kind: "loading" } | { kind: "failed" } | { kind: "loaded"; retention: Retention };

type Field = "closed_event_months" | "source_months";
type Texts = Record<Field, string>;
type Errors = Partial<Record<Field, string>>;

const FIELDS: Field[] = ["closed_event_months", "source_months"];

export interface RetentionSettingsProps {
  api: Api;
  /** The server decides; an admin reads the periods, a member sees a note only. */
  role: OrganizationRole | undefined;
  /** The page owns the live regions: a result goes to its status region. */
  onStatus: (message: string | undefined) => void;
  /** A failure goes to the alert region of the page. */
  onFailure: (message: string | undefined) => void;
}

const textsOf = (retention: Retention): Texts => ({
  closed_event_months: retention.closed_event_months?.toString() ?? "",
  source_months: retention.source_months?.toString() ?? "",
});

/** Empty means off. Text that is no whole number is `NaN`; the server checks the range. */
function monthsOf(text: string): number | null {
  const trimmed = text.trim();
  return trimmed === "" ? null : /^\d+$/.test(trimmed) ? Number(trimmed) : Number.NaN;
}

/**
 * The retention periods on the page of the organization (ADR 0071). An owner sets them, an admin
 * reads them. Empty means „keine automatische Löschung“. The note names the accounting duty,
 * because sources can hold invoices and contracts.
 */
export function RetentionSettings({ api, role, onStatus, onFailure }: RetentionSettingsProps) {
  const isOwner = role === "owner";
  return (
    <section className={styles.section} aria-labelledby="org-retention-title">
      <h3 id="org-retention-title" tabIndex={-1} className={styles.title}>
        {t("org-retention-title")}
      </h3>
      {canManage(role) ? (
        <Periods api={api} isOwner={isOwner} onStatus={onStatus} onFailure={onFailure} />
      ) : (
        <p className={styles.help}>{t("org-retention-owner-only-hidden")}</p>
      )}
    </section>
  );
}

function Periods({
  api,
  isOwner,
  onStatus,
  onFailure,
}: {
  api: Api;
  isOwner: boolean;
  onStatus: RetentionSettingsProps["onStatus"];
  onFailure: RetentionSettingsProps["onFailure"];
}) {
  const [state, setState] = useState<State>({ kind: "loading" });
  const [texts, setTexts] = useState<Texts>({ closed_event_months: "", source_months: "" });
  const [errors, setErrors] = useState<Errors>({});
  const [busy, setBusy] = useState(false);
  // A change of the periods needs a recent sign-in (ADR 0071): the form then offers a new one.
  const [signInAgain, setSignInAgain] = useState(false);
  const form = useRef<HTMLFormElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  const { retried, retry } = useRetry(() => document.getElementById("org-retention-title"));

  /** Resolves to true when the periods loaded. */
  const load = useCallback(async () => {
    try {
      const { data } = await api.GET("/api/v1/organization/retention");
      if (data) {
        setState({ kind: "loaded", retention: data });
        setTexts(textsOf(data));
        return true;
      }
    } catch {
      // The same failure state as a problem response.
    }
    setState({ kind: "failed" });
    return false;
  }, [api]);

  useEffect(() => {
    void load();
  }, [load]);

  const rangeError = t("org-retention-error-out-of-range");

  const save = async (event: FormEvent) => {
    event.preventDefault();
    if (busy || state.kind !== "loaded") {
      return;
    }
    onFailure(undefined);
    onStatus(undefined);
    const months = {
      closed_event_months: monthsOf(texts.closed_event_months),
      source_months: monthsOf(texts.source_months),
    };
    const local = Object.fromEntries(
      FIELDS.filter((field) => Number.isNaN(months[field])).map((field) => [field, rangeError]),
    );
    setErrors(local);
    if (Object.keys(local).length > 0) {
      focusAfterCommit(() => firstInvalidField(form.current));
      return;
    }
    setBusy(true);
    setSignInAgain(false);
    try {
      const result = await api.POST("/api/v1/organization/retention/set", {
        body: { ...months, expected_version: state.retention.version },
      });
      if (result.data) {
        setState({ kind: "loaded", retention: result.data });
        setTexts(textsOf(result.data));
        onStatus(t("org-retention-saved"));
      } else if (result.error?.code === "validation-failed") {
        const next: Errors = {};
        for (const field of FIELDS) {
          const code = result.error.errors?.find((e) => e.pointer === `/${field}`)?.code;
          if (code) {
            next[field] = hasMessage(`org-retention-error-${code}`)
              ? t(`org-retention-error-${code}`)
              : t("problem-validation-failed");
          }
        }
        setErrors(next);
        focusAfterCommit(() => firstInvalidField(form.current));
      } else if (result.error?.code === "record-version-conflict") {
        // Someone else changed the periods: the form shows the truth and the new version.
        await load();
        onFailure(t("org-retention-conflict"));
      } else if (result.error?.code === "recent-sign-in-required") {
        setSignInAgain(true);
        onFailure(problemMessage(result.error));
      } else {
        onFailure(failureOf(result).message);
      }
    } catch {
      onFailure(problemMessage(undefined));
    }
    setBusy(false);
  };

  if (state.kind === "loading") {
    return (
      <div className={styles.skeleton} role="status" aria-label={t("org-retention-loading")}>
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

  const field = (name: Field, label: string, help: string) => (
    <TextField
      label={label}
      help={help}
      value={texts[name]}
      onChange={(value) => {
        setTexts({ ...texts, [name]: value });
        setErrors({ ...errors, [name]: undefined });
      }}
      error={errors[name]}
      inputMode="numeric"
      isDisabled={!isOwner}
    />
  );
  return (
    <>
      <p className={styles.help}>{t("org-retention-intro")}</p>
      <p className={styles.note}>{t("org-retention-accounting")}</p>
      <form ref={form} className={styles.form} onSubmit={(event) => void save(event)} noValidate>
        {field(
          "closed_event_months",
          t("org-retention-closed-label"),
          t("org-retention-closed-help"),
        )}
        {field("source_months", t("org-retention-source-label"), t("org-retention-source-help"))}
        {isOwner ? (
          <div className={styles.actions}>
            <Button type="submit" variant="primary" isPending={busy}>
              {t("org-retention-save")}
            </Button>
          </div>
        ) : (
          <p className={styles.help}>{t("org-retention-owner-only")}</p>
        )}
      </form>
      {signInAgain && <SignInAgain />}
    </>
  );
}
