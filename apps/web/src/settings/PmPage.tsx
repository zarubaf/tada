import { IconPlayerPause } from "@tabler/icons-react";
import { type Ref, useCallback, useEffect, useRef, useState } from "react";
import pmMessages from "../../../../locales/de-CH/ai-pm.ftl?raw";
import {
  type AiStatus,
  type Api,
  type PmPolicy,
  type Problem,
  problemMessage,
} from "../api/client";
import { failureOf } from "../api/failure";
import { addMessages, t } from "../i18n";
import { canManage } from "../members/roles";
import { useSession } from "../session/SessionProvider";
import { Banner } from "../ui/Banner";
import { useRetry } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { LiveRegion } from "../ui/LiveRegion";
import { NoAccess } from "../ui/NoAccess";
import { PanelTitle } from "../ui/Page";
import { SkeletonLines } from "../ui/Skeleton";
import { Switch } from "../ui/Switch";
import { PmAiSettings } from "./PmAiSettings";
import { PmDigests } from "./PmDigests";
import styles from "./PmPage.module.css";
import { PmPolicyForm } from "./PmPolicyForm";
import { type PolicyValues, valuesWith } from "./pmPolicy";

// The texts of the page load with its chunk, not with the first load (ADR 0024).
addMessages(pmMessages);

type State =
  | { kind: "loading" }
  | { kind: "failed"; problem: Problem | undefined }
  | { kind: "loaded"; policy: PmPolicy; ai: AiStatus };

/** The answer of a save of the policy, for the part of the page that saved. */
export type PolicySave =
  | { kind: "saved"; policy: PmPolicy }
  | { kind: "invalid"; problem: Problem }
  | { kind: "conflict"; policy: PmPolicy | undefined }
  | { kind: "failed" };

const PAUSE_HELP = "pm-pause-help";

/**
 * „AI-PM“ in the settings (ADR 0076): the policy of the reminders, the pause for all messages, the
 * AI switch with the monthly cap, and the last digests. An owner or an admin reads and changes them.
 * Any other member sees the no-access state and the page sends no request. The server decides: a
 * `forbidden` answer, for example after a change of the role, shows the same state. The switches
 * have an immediate effect; the other values save with the form.
 */
export function PmPage({ api }: { api: Api }) {
  const role = useSession().organization?.role;
  return canManage(role) ? (
    <PmSettings api={api} />
  ) : (
    <div className={styles.page}>
      <PmHeader />
      <PmNoAccess />
    </div>
  );
}

function PmHeader({ title }: { title?: Ref<HTMLHeadingElement> }) {
  return (
    <header className={styles.header}>
      <PanelTitle ref={title}>{t("pm-title")}</PanelTitle>
      <p className={styles.intro}>{t("pm-intro")}</p>
    </header>
  );
}

/** The no-access state in the width of the header. */
function PmNoAccess() {
  return (
    <div className={styles.noAccess}>
      <NoAccess />
    </div>
  );
}

function PmSettings({ api }: { api: Api }) {
  const [state, setState] = useState<State>({ kind: "loading" });
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string>();
  const [confirmation, setConfirmation] = useState<string>();
  // A change of the AI switch needs a recent sign-in (ADR 0077).
  const [signInAgain, setSignInAgain] = useState(false);
  const title = useRef<HTMLHeadingElement>(null);
  const { retried, retry } = useRetry(() => title.current);

  /** Resolves to true when the policy and the AI state loaded. */
  const load = useCallback(async () => {
    try {
      const [policy, ai] = await Promise.all([
        api.GET("/api/v1/organization/pm-policy"),
        api.GET("/api/v1/organization/ai"),
      ]);
      if (policy.data && ai.data) {
        setState({ kind: "loaded", policy: policy.data, ai: ai.data });
        return true;
      }
      setState({ kind: "failed", problem: policy.error ?? ai.error });
    } catch {
      setState({ kind: "failed", problem: undefined });
    }
    return false;
  }, [api]);

  useEffect(() => {
    void load();
  }, [load]);

  /** Reads the policy again, for example after a version conflict. */
  const reloadPolicy = useCallback(async () => {
    try {
      const { data } = await api.GET("/api/v1/organization/pm-policy");
      if (data) {
        setState((old) => (old.kind === "loaded" ? { ...old, policy: data } : old));
        return data;
      }
    } catch {
      // The page keeps the old values; the message of the conflict says what happened.
    }
    return undefined;
  }, [api]);

  /** Saves the whole policy. The caller shows the result; this function owns the busy state. */
  const savePolicy = useCallback(
    async (values: PolicyValues, expectedVersion: number): Promise<PolicySave> => {
      setFailure(undefined);
      setConfirmation(undefined);
      setSignInAgain(false);
      setBusy(true);
      try {
        const result = await api.POST("/api/v1/organization/pm-policy/set", {
          body: { ...values, expected_version: expectedVersion },
        });
        if (result.data) {
          const policy = result.data;
          setState((old) => (old.kind === "loaded" ? { ...old, policy } : old));
          return { kind: "saved", policy };
        }
        if (result.error?.code === "validation-failed") {
          return { kind: "invalid", problem: result.error };
        }
        if (result.error?.code === "record-version-conflict") {
          // Someone else changed the policy: the page shows the truth and the new version.
          return { kind: "conflict", policy: await reloadPolicy() };
        }
        if (result.error?.code === "recent-sign-in-required") {
          setSignInAgain(true);
          setFailure(problemMessage(result.error));
          return { kind: "failed" };
        }
        setFailure(failureOf(result).message);
      } catch {
        setFailure(problemMessage(undefined));
      } finally {
        setBusy(false);
      }
      return { kind: "failed" };
    },
    [api, reloadPolicy],
  );

  const toggle = async (
    change: Pick<PolicyValues, "paused"> | Pick<PolicyValues, "ai_enabled">,
  ) => {
    if (busy || state.kind !== "loaded") {
      return;
    }
    const saved = await savePolicy(valuesWith(state.policy, change), state.policy.version);
    if (saved.kind === "saved") {
      setConfirmation(
        "paused" in change
          ? t(change.paused ? "pm-paused" : "pm-resumed")
          : t(change.ai_enabled ? "pm-ai-on" : "pm-ai-off"),
      );
    } else if (saved.kind === "invalid") {
      setFailure(problemMessage(saved.problem));
    } else if (saved.kind === "conflict") {
      // The switches show the new values, but the form keeps unsaved edits of the member.
      setFailure(t(saved.policy ? "pm-conflict-switch" : "pm-conflict-reload-failed"));
    }
  };

  return (
    <div className={styles.page}>
      <LiveRegion kind="alert">{failure}</LiveRegion>
      <LiveRegion kind="status">{confirmation}</LiveRegion>
      <PmHeader title={title} />
      {state.kind === "loading" && <SkeletonLines label={t("pm-loading")} />}
      {state.kind === "failed" &&
        (state.problem?.code === "forbidden" ? (
          // The role changed after the sign-in: a retry gains nothing.
          <PmNoAccess />
        ) : (
          <InlineError
            message={problemMessage(state.problem)}
            requestId={state.problem?.request_id}
            onRetry={() =>
              retry(() => {
                setState({ kind: "loading" });
                return load();
              })
            }
            announce={retried ? "focus" : "alert"}
          />
        ))}
      {state.kind === "loaded" && (
        <>
          <div className={styles.pauseArea}>
            {state.policy.paused && (
              <Banner icon={IconPlayerPause}>
                <strong>{t("pm-paused-title")}</strong>
                <p>{t("pm-paused-text")}</p>
              </Banner>
            )}
            <div className={styles.pause}>
              <Switch
                label={t("pm-pause")}
                isSelected={state.policy.paused}
                onChange={(paused) => void toggle({ paused })}
                isPending={busy}
                aria-describedby={PAUSE_HELP}
              />
              <p id={PAUSE_HELP} className={styles.help}>
                {t(state.policy.paused ? "pm-pause-help-on" : "pm-pause-help-off")}
              </p>
            </div>
          </div>
          <PmPolicyForm
            policy={state.policy}
            busy={busy}
            onSave={savePolicy}
            onStatus={setConfirmation}
            onFailure={setFailure}
          />
          <PmAiSettings
            api={api}
            ai={state.ai}
            aiEnabled={state.policy.ai_enabled}
            busy={busy}
            signInAgain={signInAgain}
            onToggle={(ai_enabled) => void toggle({ ai_enabled })}
            onAi={(ai) => setState((old) => (old.kind === "loaded" ? { ...old, ai } : old))}
            onStatus={setConfirmation}
            onFailure={setFailure}
          />
          <PmDigests api={api} timeZone={state.policy.time_zone} />
        </>
      )}
    </div>
  );
}
