import { IconAlertTriangle } from "@tabler/icons-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { type Api, problemMessage, type SystemVersion } from "../api/client";
import { LOCALE, t } from "../i18n";
import { DataTable } from "../ui/DataTable";
import { useRetry } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { Sheet } from "../ui/Sheet";
import { SkeletonLines } from "../ui/Skeleton";
import { StatusLabel } from "../ui/StatusLabel";
import styles from "./ComponentList.module.css";
import { CLIENT_BUILD_ID, outdated } from "./version";

const seenFormat = new Intl.DateTimeFormat(LOCALE, { dateStyle: "short", timeStyle: "short" });

type State =
  | { kind: "loading" }
  | { kind: "failed"; message: string; requestId: string | undefined }
  | { kind: "loaded"; version: SystemVersion };

interface Row {
  id: string;
  name: string;
  version: string;
  /** The component runs another build than the server that answers. */
  differs?: boolean;
}

interface BuildIdentity {
  version: string;
  commit: string;
}

/** `0.3.0 · 1a2b3c4`; „unbekannt“ for a role of a release before its heartbeat had a version. */
function buildText({ version, commit }: BuildIdentity): string {
  return version === "unknown" ? t("versions-unknown") : `${version} · ${commit}`;
}

/**
 * True if a role runs another build than the server. A build without Git gives the commit
 * `unknown`; then only the versions count.
 */
function differs(server: BuildIdentity, role: BuildIdentity): boolean {
  const commitsKnown = server.commit !== "unknown" && role.commit !== "unknown";
  return server.version !== role.version || (commitsKnown && server.commit !== role.commit);
}

function componentsOf(version: SystemVersion): Row[] {
  const server = { version: version.product, commit: version.commit };
  const old = outdated(version);
  return [
    {
      id: "web",
      name: t("versions-web"),
      // The client knows only its opaque build ID (ADR 0078). It is the build of the server, or another one.
      version: old ? CLIENT_BUILD_ID : buildText(server),
      differs: old,
    },
    { id: "server", name: t("versions-server"), version: buildText(server) },
    { id: "schema", name: t("versions-schema"), version: String(version.schema) },
    { id: "api", name: t("versions-api"), version: version.api },
    { id: "mcp", name: t("versions-mcp"), version: version.mcp },
  ];
}

function VersionCell({ row }: { row: Row }) {
  return (
    <span className={styles.version}>
      {row.version}
      {row.differs && (
        <StatusLabel icon={IconAlertTriangle} tone="warning">
          {t("versions-differs")}
        </StatusLabel>
      )}
    </span>
  );
}

/**
 * The versions of tada and its components (ADR 0075), in a sheet that the footer opens. The list
 * loads when the sheet opens, so that the heartbeats are current. It marks each component that
 * runs another build than the server.
 */
export function ComponentList({ api, onClose }: { api: Api; onClose: () => void }) {
  const [state, setState] = useState<State>({ kind: "loading" });
  const list = useRef<HTMLDivElement>(null);
  const { retried, retry } = useRetry(() => list.current);

  const load = useCallback(async () => {
    try {
      const { data, error } = await api.GET("/api/v1/system/version");
      if (data) {
        setState({ kind: "loaded", version: data });
        return true;
      }
      setState({ kind: "failed", message: problemMessage(error), requestId: error?.request_id });
    } catch {
      setState({ kind: "failed", message: problemMessage(undefined), requestId: undefined });
    }
    return false;
  }, [api]);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <Sheet title={t("versions-title")} onClose={onClose}>
      {state.kind === "loading" && <SkeletonLines label={t("versions-loading")} />}
      {state.kind === "failed" && (
        <InlineError
          message={state.message}
          requestId={state.requestId}
          onRetry={() =>
            retry(() => {
              setState({ kind: "loading" });
              return load();
            })
          }
          announce={retried ? "focus" : "alert"}
        />
      )}
      {state.kind === "loaded" && (
        <div ref={list} tabIndex={-1} className={styles.list}>
          <section className={styles.section} aria-labelledby="versions-components">
            <h3 id="versions-components" className={styles.heading}>
              {t("versions-components")}
            </h3>
            <DataTable
              label={t("versions-components")}
              rows={componentsOf(state.version)}
              rowKey={(row) => row.id}
              columns={[
                { id: "name", header: t("versions-component"), cell: (row) => row.name },
                {
                  id: "version",
                  header: t("versions-version"),
                  cell: (row) => <VersionCell row={row} />,
                },
              ]}
            />
          </section>
          <section className={styles.section} aria-labelledby="versions-roles">
            <h3 id="versions-roles" className={styles.heading}>
              {t("versions-roles")}
            </h3>
            <p className={styles.help}>{t("versions-roles-help")}</p>
            {state.version.roles.length === 0 ? (
              <p>{t("versions-no-roles")}</p>
            ) : (
              <DataTable
                label={t("versions-roles")}
                rows={state.version.roles}
                rowKey={(role) => role.role}
                columns={[
                  { id: "role", header: t("versions-role"), mono: true, cell: (role) => role.role },
                  {
                    id: "version",
                    header: t("versions-version"),
                    cell: (role) => (
                      <VersionCell
                        row={{
                          id: role.role,
                          name: role.role,
                          version: buildText(role),
                          differs: differs(
                            { version: state.version.product, commit: state.version.commit },
                            role,
                          ),
                        }}
                      />
                    ),
                  },
                  {
                    id: "seen",
                    header: t("versions-last-seen"),
                    numeric: true,
                    cell: (role) => seenFormat.format(new Date(role.last_seen)),
                  },
                ]}
              />
            )}
          </section>
        </div>
      )}
    </Sheet>
  );
}
