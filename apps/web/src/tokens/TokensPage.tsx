import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";
import {
  type Api,
  type ApiToken,
  type ApiTokenScope,
  type Problem,
  problemMessage,
} from "../api/client";
import { failureOf } from "../api/failure";
import { LOCALE, t } from "../i18n";
import { loadMcpSwitch } from "../settings/mcpSwitch";
import { Button } from "../ui/Button";
import { Checkbox } from "../ui/Checkbox";
import { ConfirmDialog } from "../ui/ConfirmDialog";
import { DataTable } from "../ui/DataTable";
import { EmptyState } from "../ui/EmptyState";
import { firstInvalidField, useFocusAfterCommit, useRetry } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { LiveRegion } from "../ui/LiveRegion";
import { PageTitle } from "../ui/Page";
import { Select } from "../ui/Select";
import { Skeleton } from "../ui/Skeleton";
import { TextField } from "../ui/TextField";
import styles from "./TokensPage.module.css";

const dateFormat = new Intl.DateTimeFormat(LOCALE, { dateStyle: "medium" });
const dateTimeFormat = new Intl.DateTimeFormat(LOCALE, { dateStyle: "medium", timeStyle: "short" });

const DAY = 24 * 60 * 60 * 1000;
/** The server allows an expiry at most one year ahead; 364 days stay inside it. */
const EXPIRIES = [30, 90, 180, 364] as const;
const SCOPES: ApiTokenScope[] = ["read", "propose"];
const MCP_PATH = "/mcp";

type List =
  | { kind: "loading" }
  | { kind: "failed"; message: string; requestId: string | undefined }
  | { kind: "loaded"; items: ApiToken[] };

/** The state of the MCP switch as far as this page knows it. */
type McpSwitch = "unknown" | "on" | "off";

function statusOf(token: ApiToken, now: number): "revoked" | "expired" | "active" {
  if (token.revoked_at) {
    return "revoked";
  }
  return new Date(token.expires_at).getTime() <= now ? "expired" : "active";
}

/** The client examples. The address comes from the page, so that no host is written into the code. */
function examples(origin: string) {
  const url = `${origin}${MCP_PATH}`;
  return {
    claude: `claude mcp add --transport http tada ${url} \\\n  --header "Authorization: Bearer TOKEN"`,
    codex: `[mcp_servers.tada]\nurl = "${url}"\nbearer_token_env_var = "TADA_TOKEN"`,
  };
}

/**
 * „API-Token“ in the settings (ADR 0045, ADR 0039): the member reads the notice, confirms it,
 * creates a token and sees its secret once. The secret lives only in the state of this page.
 * It never reaches storage, the address or a cache, and it is gone when the member leaves.
 */
export function TokensPage({ api }: { api: Api }) {
  const [list, setList] = useState<List>({ kind: "loading" });
  const [noticeVersion, setNoticeVersion] = useState<number>();
  const [noticeFailure, setNoticeFailure] = useState<{
    message: string;
    requestId: string | undefined;
  }>();
  const [mcp, setMcp] = useState<McpSwitch>("unknown");
  const [name, setName] = useState("");
  const [scope, setScope] = useState<ApiTokenScope>("read");
  const [expiry, setExpiry] = useState<string>("90");
  const [confirmed, setConfirmed] = useState(false);
  const [nameError, setNameError] = useState<string>();
  const [secret, setSecret] = useState<string>();
  const [revoking, setRevoking] = useState<ApiToken>();
  const [busy, setBusy] = useState(false);
  // Both live regions are in the page from the start, so that a text set later is announced.
  const [failure, setFailure] = useState<string>();
  const [confirmation, setConfirmation] = useState<string>();
  const alert = useRef<HTMLParagraphElement>(null);
  const form = useRef<HTMLFormElement>(null);
  const secretBox = useRef<HTMLDivElement>(null);
  const listHeading = useRef<HTMLHeadingElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  const noticeHeading = useRef<HTMLHeadingElement>(null);
  const { retried, retry } = useRetry(() => listHeading.current);
  const noticeRetry = useRetry(() => noticeHeading.current);

  /** Resolves to true when the list loaded. A failure keeps a loaded list. */
  const loadList = useCallback(async () => {
    let failed: { message: string; requestId: string | undefined };
    try {
      const { data, error } = await api.GET("/api/v1/tokens");
      if (data) {
        setList({ kind: "loaded", items: data.items });
        return true;
      }
      failed = { message: problemMessage(error), requestId: error?.request_id };
    } catch {
      failed = { message: problemMessage(undefined), requestId: undefined };
    }
    setList((current) => (current.kind === "loaded" ? current : { kind: "failed", ...failed }));
    return false;
  }, [api]);

  const loadMcp = useCallback(async () => {
    const feature = await loadMcpSwitch(api);
    // An unknown state does not block: the server decides.
    setMcp(feature === undefined ? "unknown" : feature.enabled ? "on" : "off");
    return feature;
  }, [api]);

  /** Resolves to true when the version of the notice loaded. */
  const loadNotice = useCallback(async () => {
    try {
      const { data, error } = await api.GET("/api/v1/token-notice");
      if (data) {
        setNoticeVersion(data.version);
        setNoticeFailure(undefined);
        return true;
      }
      setNoticeFailure({ message: problemMessage(error), requestId: error?.request_id });
    } catch {
      setNoticeFailure({ message: problemMessage(undefined), requestId: undefined });
    }
    return false;
  }, [api]);

  useEffect(() => {
    void loadList();
    void loadMcp();
    void loadNotice();
  }, [loadList, loadMcp, loadNotice]);

  const showFailure = (message: string) => {
    setFailure(message);
    // The failure may be far above the button that the member pressed.
    alert.current?.scrollIntoView?.({ block: "nearest" });
  };

  /** The message of a refused create call: the switch, the missing right, or a field problem. */
  const createFailure = async (
    error: Problem | undefined,
    result: Parameters<typeof failureOf>[0],
  ) => {
    if (error?.code === "forbidden") {
      // Only the switch tells the two reasons of a refusal apart. Without it, the text stays neutral.
      const feature = await loadMcp();
      showFailure(
        feature === undefined
          ? problemMessage(error)
          : feature.enabled
            ? t("tokens-propose-forbidden")
            : t("tokens-off"),
      );
      return;
    }
    if (error?.code === "validation-failed") {
      const pointers = (error.errors ?? []).map((e) => e.pointer);
      if (pointers.includes("/name")) {
        setNameError(t("tokens-error-name"));
        focusAfterCommit(() => firstInvalidField(form.current));
        return;
      }
      if (pointers.includes("/notice_version_confirmed")) {
        setConfirmed(false);
        showFailure(t("tokens-error-notice"));
        return;
      }
    }
    showFailure(failureOf(result).message);
  };

  const create = async (event: FormEvent) => {
    event.preventDefault();
    if (busy || !confirmed || noticeVersion === undefined) {
      return;
    }
    const trimmed = name.trim();
    if (trimmed === "" || trimmed.length > 100) {
      setNameError(t(trimmed === "" ? "tokens-error-name" : "tokens-error-name-too-long"));
      focusAfterCommit(() => firstInvalidField(form.current));
      return;
    }
    setNameError(undefined);
    setFailure(undefined);
    setConfirmation(undefined);
    setBusy(true);
    try {
      const result = await api.POST("/api/v1/tokens", {
        body: {
          name: trimmed,
          scope,
          expires_at: new Date(Date.now() + Number(expiry) * DAY).toISOString(),
          notice_version_confirmed: noticeVersion,
        },
      });
      if (result.data) {
        setSecret(result.data.secret);
        setName("");
        // Each token needs its own confirmation.
        setConfirmed(false);
        setConfirmation(t("tokens-created"));
        // Focus goes to the secret, so that a screen reader reads it.
        focusAfterCommit(() => secretBox.current);
        void loadList();
      } else {
        await createFailure(result.error, result);
      }
    } catch {
      showFailure(problemMessage(undefined));
    }
    setBusy(false);
  };

  const copy = async () => {
    setFailure(undefined);
    setConfirmation(undefined);
    try {
      await navigator.clipboard.writeText(secret ?? "");
      setConfirmation(t("tokens-copied"));
    } catch {
      showFailure(t("tokens-copy-failed"));
    }
  };

  const revoke = async () => {
    if (busy || !revoking) {
      return;
    }
    const item = revoking;
    setFailure(undefined);
    setConfirmation(undefined);
    setBusy(true);
    try {
      const result = await api.POST("/api/v1/tokens/{token_id}/revoke", {
        params: { path: { token_id: item.id } },
      });
      if (result.response.ok) {
        // The pressed button leaves the row: focus goes to the heading of the list.
        focusAfterCommit(() => listHeading.current);
        setConfirmation(t("tokens-revoked"));
        await loadList();
      } else {
        showFailure(failureOf(result).message);
      }
    } catch {
      showFailure(problemMessage(undefined));
    }
    setBusy(false);
    setRevoking(undefined);
  };

  const now = Date.now();
  const sample = examples(globalThis.location.origin);
  const canCreate = confirmed && mcp !== "off" && noticeVersion !== undefined;

  return (
    <div className={styles.page}>
      <LiveRegion ref={alert} kind="alert">
        {failure}
      </LiveRegion>
      <LiveRegion kind="status">{confirmation}</LiveRegion>

      <section className={styles.section} aria-labelledby="tokens-title">
        <PageTitle id="tokens-title">{t("tokens-title")}</PageTitle>
        <p>{t("tokens-intro")}</p>
        <div className={styles.notice}>
          <h2 ref={noticeHeading} tabIndex={-1} className={styles.heading}>
            {t("token-notice-title")}
          </h2>
          <p>{t("token-notice-access")}</p>
          <p>{t("token-notice-policy")}</p>
        </div>
        {noticeFailure && (
          <InlineError
            message={noticeFailure.message}
            requestId={noticeFailure.requestId}
            onRetry={() => noticeRetry.retry(loadNotice)}
            announce={noticeRetry.retried ? "focus" : failure ? "none" : "alert"}
          />
        )}
        {mcp === "off" && <p className={styles.off}>{t("tokens-off")}</p>}

        <h2 className={styles.heading}>{t("tokens-create-title")}</h2>
        <form
          ref={form}
          className={styles.form}
          onSubmit={(event) => void create(event)}
          noValidate
        >
          <TextField
            label={t("tokens-name")}
            help={t("tokens-name-help")}
            value={name}
            onChange={(value) => {
              setName(value);
              setNameError(undefined);
            }}
            error={nameError}
            autoComplete="off"
            isRequired
          />
          <Select
            label={t("tokens-scope")}
            options={SCOPES.map((s) => ({ id: s, label: t(`tokens-scope-${s}`) }))}
            value={scope}
            onChange={(selected) => setScope(selected as ApiTokenScope)}
          />
          <Select
            label={t("tokens-expiry")}
            options={EXPIRIES.map((days) => ({
              id: String(days),
              label: t(`tokens-expiry-${days}`),
            }))}
            value={expiry}
            onChange={setExpiry}
          />
          <Checkbox
            label={t("token-notice-confirm")}
            isSelected={confirmed}
            onChange={setConfirmed}
          />
          <div>
            <Button type="submit" variant="primary" isPending={busy} isDisabled={!canCreate}>
              {t("tokens-create")}
            </Button>
          </div>
        </form>

        {secret !== undefined && (
          <div ref={secretBox} tabIndex={-1} className={styles.secret}>
            <p className={styles.secretLabel}>{t("tokens-secret-label")}</p>
            <p className={styles.secretValue}>{secret}</p>
            <p className={styles.once}>{t("tokens-secret-once")}</p>
            <p>{t("tokens-secret-hint")}</p>
            <div>
              <Button onPress={() => void copy()}>{t("tokens-copy")}</Button>
            </div>
          </div>
        )}
      </section>

      <section className={`${styles.section} ${styles.wide}`} aria-labelledby="tokens-list-title">
        <h2 id="tokens-list-title" ref={listHeading} tabIndex={-1} className={styles.heading}>
          {t("tokens-list-title")}
        </h2>
        {list.kind === "loading" && (
          <div className={styles.skeleton} role="status" aria-label={t("tokens-loading")}>
            <Skeleton />
            <Skeleton />
          </div>
        )}
        {list.kind === "failed" && (
          <InlineError
            message={list.message}
            requestId={list.requestId}
            onRetry={() =>
              retry(async () => {
                setFailure(undefined);
                setList({ kind: "loading" });
                return loadList();
              })
            }
            announce={retried ? "focus" : failure ? "none" : "alert"}
          />
        )}
        {list.kind === "loaded" && list.items.length === 0 && (
          <EmptyState title={t("tokens-empty-title")} text={t("tokens-empty-text")} />
        )}
        {list.kind === "loaded" && list.items.length > 0 && (
          <DataTable
            label={t("tokens-list-title")}
            columns={[
              { id: "name", header: t("tokens-column-name"), cell: (token) => token.name },
              {
                id: "scope",
                header: t("tokens-column-scope"),
                cell: (token) => t(`tokens-scope-${token.scope}`),
              },
              {
                id: "expires",
                header: t("tokens-column-expires"),
                cell: (token) => (
                  <time dateTime={token.expires_at}>
                    {dateFormat.format(new Date(token.expires_at))}
                  </time>
                ),
                numeric: true,
              },
              {
                id: "last-used",
                header: t("tokens-column-last-used"),
                cell: (token) =>
                  token.last_used_at ? (
                    <time dateTime={token.last_used_at}>
                      {dateTimeFormat.format(new Date(token.last_used_at))}
                    </time>
                  ) : (
                    t("tokens-never-used")
                  ),
                numeric: true,
              },
              {
                id: "status",
                header: t("tokens-column-status"),
                cell: (token) => t(`tokens-status-${statusOf(token, now)}`),
              },
              {
                id: "actions",
                header: t("tokens-column-actions"),
                cell: (token) =>
                  statusOf(token, now) === "active" ? (
                    <Button
                      variant="danger"
                      aria-label={t("tokens-revoke-of", { name: token.name })}
                      onPress={() => setRevoking(token)}
                    >
                      {t("tokens-revoke")}
                    </Button>
                  ) : null,
              },
            ]}
            rows={list.items}
            rowKey={(token) => token.id}
          />
        )}
      </section>

      <section className={styles.section} aria-labelledby="tokens-example-title">
        <h2 id="tokens-example-title" className={styles.heading}>
          {t("tokens-example-title")}
        </h2>
        <p>{t("tokens-example-intro")}</p>
        <h3 className={styles.subheading}>{t("tokens-example-claude")}</h3>
        <pre className={styles.code}>
          <code>{sample.claude}</code>
        </pre>
        <h3 className={styles.subheading}>{t("tokens-example-codex")}</h3>
        <p>{t("tokens-example-codex-text")}</p>
        <pre className={styles.code}>
          <code>{sample.codex}</code>
        </pre>
      </section>

      <ConfirmDialog
        isOpen={revoking !== undefined}
        title={t("tokens-revoke-title")}
        text={t("tokens-revoke-text", { name: revoking?.name ?? "" })}
        confirmLabel={t("tokens-revoke")}
        cancelLabel={t("tokens-revoke-cancel")}
        isPending={busy}
        onConfirm={() => void revoke()}
        onCancel={() => !busy && setRevoking(undefined)}
      />
    </div>
  );
}
