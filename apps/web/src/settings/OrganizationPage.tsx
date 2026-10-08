import { useCallback, useEffect, useRef, useState } from "react";
import { type Api, type OrganizationFeature, problemMessage } from "../api/client";
import { failureOf } from "../api/failure";
import { t } from "../i18n";
import { useSession } from "../session/SessionProvider";
import { useRetry } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { LiveRegion } from "../ui/LiveRegion";
import { PageTitle } from "../ui/Page";
import { Skeleton } from "../ui/Skeleton";
import { Switch } from "../ui/Switch";
import { loadMcpSwitch } from "./mcpSwitch";
import styles from "./OrganizationPage.module.css";

type State =
  | { kind: "loading" }
  | { kind: "failed" }
  | { kind: "loaded"; feature: OrganizationFeature };

/**
 * The settings of the organization. Today: the switch for MCP tokens (ADR 0045), which only an
 * owner changes. The server decides; the switch of a member is disabled so that it does not fail.
 */
export function OrganizationPage({ api }: { api: Api }) {
  const session = useSession();
  const isOwner = session.organization?.role === "owner";
  const [state, setState] = useState<State>({ kind: "loading" });
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string>();
  const [confirmation, setConfirmation] = useState<string>();
  const title = useRef<HTMLHeadingElement>(null);
  const { retried, retry } = useRetry(() => title.current);

  const load = useCallback(async () => {
    const feature = await loadMcpSwitch(api);
    setState(feature ? { kind: "loaded", feature } : { kind: "failed" });
    return feature !== undefined;
  }, [api]);

  useEffect(() => {
    void load();
  }, [load]);

  const change = async (enabled: boolean) => {
    if (busy || state.kind !== "loaded") {
      return;
    }
    setFailure(undefined);
    setConfirmation(undefined);
    setBusy(true);
    try {
      const result = await api.POST("/api/v1/organization/features/{feature}/set", {
        params: { path: { feature: "mcp-tokens" } },
        body: { enabled, expected_version: state.feature.version },
      });
      if (result.data) {
        setState({ kind: "loaded", feature: result.data });
        setConfirmation(t(result.data.enabled ? "org-mcp-on" : "org-mcp-off"));
      } else {
        setFailure(failureOf(result).message);
        if (result.error?.code === "record-version-conflict") {
          // Someone else changed the switch: it shows the truth again.
          void load();
        }
      }
    } catch {
      setFailure(problemMessage(undefined));
    }
    setBusy(false);
  };

  return (
    <div className={styles.page}>
      <LiveRegion kind="alert">{failure}</LiveRegion>
      <LiveRegion kind="status">{confirmation}</LiveRegion>
      <section className={styles.section} aria-labelledby="org-title">
        <PageTitle id="org-title" ref={title}>
          {t("org-title")}
        </PageTitle>
        {state.kind === "loading" && (
          <div className={styles.skeleton} role="status" aria-label={t("org-mcp-loading")}>
            <Skeleton />
          </div>
        )}
        {state.kind === "failed" && (
          <InlineError
            message={problemMessage(undefined)}
            onRetry={() =>
              retry(async () => {
                setState({ kind: "loading" });
                return load();
              })
            }
            announce={retried ? "focus" : "alert"}
          />
        )}
        {state.kind === "loaded" && (
          <>
            <Switch
              label={t("org-mcp-label")}
              isSelected={state.feature.enabled}
              onChange={(enabled) => void change(enabled)}
              isDisabled={!isOwner}
              isPending={busy}
            />
            <p className={styles.help}>{t("org-mcp-help")}</p>
          </>
        )}
      </section>
    </div>
  );
}
