import { IconAlertTriangle, IconCircleCheck } from "@tabler/icons-react";
import { type FormEvent, useRef, useState } from "react";
import { type AiStatus, type Api, problemMessage } from "../api/client";
import { failureOf } from "../api/failure";
import { hasMessage, LOCALE, t } from "../i18n";
import { SignInAgain } from "../session/SignInAgain";
import { Button } from "../ui/Button";
import { firstInvalidField, useFocusAfterCommit } from "../ui/focus";
import { StatusLabel } from "../ui/StatusLabel";
import { Switch } from "../ui/Switch";
import { TextField } from "../ui/TextField";
import styles from "./PmPage.module.css";

const AI_HELP = "pm-ai-help";

const chf = new Intl.NumberFormat(LOCALE, { style: "currency", currency: "CHF" });

/** An amount of the API (`25.00`) as „CHF 25.00“. */
export const formatChf = (amount: string) => chf.format(Number(amount));

/** `40,50` as `40.50`: a member can type the decimal comma. The server checks the rest. */
const amountOf = (text: string) => text.trim().replace(",", ".");

export interface PmAiSettingsProps {
  api: Api;
  ai: AiStatus;
  /** The AI switch is part of the policy. */
  aiEnabled: boolean;
  busy: boolean;
  /** The last change of the switch needs a recent sign-in. */
  signInAgain: boolean;
  onToggle: (enabled: boolean) => void;
  onAi: (ai: AiStatus) => void;
  onStatus: (message: string | undefined) => void;
  onFailure: (message: string | undefined) => void;
}

/**
 * The AI of the AI-PM: the switch, the state of the provider, the usage of the month and the monthly
 * cap (ADR 0047, ADR 0077). The cap is a record of the organization. A change of the cap or of the
 * switch needs a recent sign-in.
 */
export function PmAiSettings({
  api,
  ai,
  aiEnabled,
  busy,
  signInAgain: switchNeedsSignIn,
  onToggle,
  onAi,
  onStatus,
  onFailure,
}: PmAiSettingsProps) {
  const [cap, setCap] = useState(ai.cap_chf);
  const [capError, setCapError] = useState<string>();
  const [saving, setSaving] = useState(false);
  const [signInAgain, setSignInAgain] = useState(false);
  const form = useRef<HTMLFormElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  const reached = Number(ai.used_chf) >= Number(ai.cap_chf);

  const show = (next: AiStatus) => {
    onAi(next);
    setCap(next.cap_chf);
  };

  const invalid = (message: string) => {
    setCapError(message);
    focusAfterCommit(() => firstInvalidField(form.current));
  };

  const save = async (event: FormEvent) => {
    event.preventDefault();
    if (saving) {
      return;
    }
    onFailure(undefined);
    onStatus(undefined);
    const amount = amountOf(cap);
    if (!/^\d+(\.\d{1,5})?$/.test(amount)) {
      invalid(t("pm-cap-error-pattern"));
      return;
    }
    if (Number(amount) <= 0) {
      invalid(t("pm-cap-error-out-of-range"));
      return;
    }
    setCapError(undefined);
    setSignInAgain(false);
    setSaving(true);
    try {
      const result = await api.POST("/api/v1/organization/ai/set-cap", {
        body: { cap_chf: amount, expected_version: ai.version },
      });
      if (result.data) {
        show(result.data);
        onStatus(t("pm-cap-saved"));
      } else if (result.error?.code === "validation-failed") {
        const code = result.error.errors?.find((e) => e.pointer === "/cap_chf")?.code;
        invalid(
          code && hasMessage(`pm-cap-error-${code}`)
            ? t(`pm-cap-error-${code}`)
            : t("problem-validation-failed"),
        );
      } else if (result.error?.code === "record-version-conflict") {
        const reloaded = await api.GET("/api/v1/organization/ai").catch(() => undefined);
        if (reloaded?.data) {
          show(reloaded.data);
        }
        // The text says only what the field shows: the new value, or still the old one.
        onFailure(t(reloaded?.data ? "pm-cap-conflict" : "pm-cap-conflict-reload-failed"));
      } else if (result.error?.code === "recent-sign-in-required") {
        setSignInAgain(true);
        onFailure(problemMessage(result.error));
      } else {
        onFailure(failureOf(result).message);
      }
    } catch {
      onFailure(problemMessage(undefined));
    }
    setSaving(false);
  };

  return (
    <section className={styles.section} aria-labelledby="pm-ai-title">
      <h2 id="pm-ai-title" className={styles.heading}>
        {t("pm-ai-title")}
      </h2>
      <div className={styles.switchGroup}>
        <Switch
          label={t("pm-ai-switch")}
          isSelected={aiEnabled}
          onChange={onToggle}
          isPending={busy}
          aria-describedby={AI_HELP}
        />
        <p id={AI_HELP} className={styles.help}>
          {t("pm-ai-help")}
        </p>
      </div>
      <dl className={styles.facts}>
        <div className={styles.fact}>
          <dt>{t("pm-ai-provider")}</dt>
          <dd>
            {ai.configured ? (
              <StatusLabel icon={IconCircleCheck} tone="success" column>
                {t("pm-ai-configured")}
              </StatusLabel>
            ) : (
              <StatusLabel icon={IconAlertTriangle} tone="warning" column>
                {t("pm-ai-not-configured")}
              </StatusLabel>
            )}
          </dd>
        </div>
        <div className={styles.fact}>
          <dt>{t("pm-usage-label")}</dt>
          <dd className={styles.amount}>
            {t("pm-usage", { used: formatChf(ai.used_chf), cap: formatChf(ai.cap_chf) })}
          </dd>
        </div>
      </dl>
      {!ai.configured && <p className={styles.help}>{t("pm-ai-not-configured-help")}</p>}
      {ai.configured && reached && <p className={styles.help}>{t("pm-cap-reached")}</p>}
      <form ref={form} className={styles.capForm} onSubmit={(event) => void save(event)} noValidate>
        <div className={styles.short}>
          <TextField
            label={t("pm-cap-label")}
            help={t("pm-cap-help")}
            value={cap}
            onChange={(value) => {
              setCap(value);
              setCapError(undefined);
            }}
            error={capError}
            inputMode="decimal"
          />
        </div>
        <div className={styles.actions}>
          <Button type="submit" isPending={saving}>
            {t("pm-cap-save")}
          </Button>
        </div>
      </form>
      {(signInAgain || switchNeedsSignIn) && <SignInAgain />}
    </section>
  );
}
