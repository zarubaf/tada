import { IconClockExclamation } from "@tabler/icons-react";
import { type KeyboardEvent, useEffect, useRef, useState } from "react";
import type { Api, OpenChangeset } from "../api/client";
import { formatDateTime } from "../facts/formatValue";
import { t } from "../i18n";
import { Link, useNavigate, useParams } from "../router/Router";
import { EmptyState } from "../ui/EmptyState";
import { useFocusAfterCommit, useRetry } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { LiveRegion } from "../ui/LiveRegion";
import { Page, PageTitle } from "../ui/Page";
import { Skeleton } from "../ui/Skeleton";
import { authorName } from "./authorName";
import { ChangesetDetail } from "./ChangesetDetail";
import { type Scopes, useEventScopes } from "./eventScopes";
import { useInbox } from "./InboxProvider";
import styles from "./ReviewInbox.module.css";
import { shortcutOf } from "./shortcuts";

function ListItem({
  item,
  scopes,
  itemRef,
}: {
  item: OpenChangeset;
  scopes: Scopes;
  itemRef: (element: HTMLElement | null) => void;
}) {
  return (
    <li ref={itemRef}>
      <Link to={`/inbox/${encodeURIComponent(item.id)}`} className={styles.item}>
        <span className={styles.itemTitle}>{scopes.title(item.event_id)}</span>
        <span className={styles.itemMeta}>
          {t("inbox-open-proposals", { count: item.open_proposals })}
        </span>
        <span className={styles.itemMeta}>
          {t("inbox-proposed-by", { author: authorName(item.author) })},{" "}
          <time dateTime={item.created_at}>
            {formatDateTime(item.created_at, scopes.timeZone(item.event_id))}
          </time>
        </span>
        {item.stale && (
          <span className={styles.stale} title={t("inbox-stale-hint")}>
            <IconClockExclamation size={16} stroke={1.5} aria-hidden="true" />
            {t("inbox-stale")}
          </span>
        )}
      </Link>
    </li>
  );
}

/**
 * „Eingang“: the Review Inbox, a list with detail (doc/design/layout-and-responsiveness.md). The
 * list holds the changesets with open proposals, oldest first. The detail holds the proposals of
 * the chosen changeset. `J` and `K` move through the list while focus is inside the page.
 */
export function ReviewInbox({ api }: { api: Api }) {
  const { state, reload } = useInbox();
  const { changesetId } = useParams();
  const navigate = useNavigate();
  const scopes = useEventScopes(api);
  const heading = useRef<HTMLHeadingElement>(null);
  const listPane = useRef<HTMLElement>(null);
  const detailPane = useRef<HTMLElement>(null);
  const entries = useRef(new Map<string, HTMLElement>());
  const focusAfterCommit = useFocusAfterCommit();
  const { retried, retry } = useRetry(() => heading.current);
  const [status, setStatus] = useState<string>();
  const [failure, setFailure] = useState<string>();

  // A message belongs to the changeset that caused it.
  // biome-ignore lint/correctness/useExhaustiveDependencies: the chosen changeset is the trigger
  useEffect(() => {
    setStatus(undefined);
    setFailure(undefined);
  }, [changesetId]);

  const items = state.kind === "loaded" ? state.items : [];

  /** Opens the changeset. Focus follows to the list entry, or to the detail where the list is hidden. */
  const open = (item: OpenChangeset) => {
    navigate(`/inbox/${encodeURIComponent(item.id)}`, { moveFocus: false });
    focusAfterCommit(() => {
      const listShows = listPane.current && getComputedStyle(listPane.current).display !== "none";
      return listShows ? entries.current.get(item.id)?.querySelector("a") : detailPane.current;
    });
  };

  const onKeyDown = (event: KeyboardEvent) => {
    const key = shortcutOf(event);
    if (key !== "j" && key !== "k") {
      return;
    }
    const index = items.findIndex((item) => item.id === changesetId);
    const next = key === "j" ? items[index + 1] : index > 0 ? items[index - 1] : undefined;
    if (next) {
      event.preventDefault();
      open(next);
    }
  };

  return (
    <Page>
      {/* The keys work where focus is inside the page; no shortcut is global (WCAG 2.1.4). */}
      {/* biome-ignore lint/a11y/noStaticElementInteractions: the handler only reads shortcut keys */}
      <div className={styles.page} onKeyDown={onKeyDown}>
        <PageTitle ref={heading}>{t("inbox-title")}</PageTitle>

        <div className={styles.frame}>
          <div className={styles.layout} data-detail={changesetId === undefined ? undefined : ""}>
            <section ref={listPane} className={styles.list} aria-label={t("inbox-list-label")}>
              {state.kind === "loading" && (
                <div className={styles.skeleton} role="status" aria-label={t("inbox-loading")}>
                  <Skeleton />
                  <Skeleton />
                  <Skeleton />
                </div>
              )}
              {state.kind === "failed" && (
                <InlineError
                  message={state.message}
                  requestId={state.requestId}
                  announce={retried ? "focus" : "alert"}
                  onRetry={() => retry(reload)}
                />
              )}
              {state.kind === "loaded" && items.length === 0 && (
                <EmptyState title={t("inbox-empty-title")} text={t("inbox-empty-text")} />
              )}
              {items.length > 0 && (
                <ul className={styles.items}>
                  {items.map((item) => (
                    <ListItem
                      key={item.id}
                      item={item}
                      scopes={scopes}
                      itemRef={(element) => {
                        if (element) {
                          entries.current.set(item.id, element);
                        } else {
                          entries.current.delete(item.id);
                        }
                      }}
                    />
                  ))}
                </ul>
              )}
              {state.kind === "loaded" && state.more && (
                <p className={styles.note}>{t("inbox-more")}</p>
              )}
            </section>

            <div className={styles.detailPane}>
              {changesetId === undefined ? (
                <p className={styles.placeholder}>{t("inbox-detail-select")}</p>
              ) : (
                <ChangesetDetail
                  key={changesetId}
                  ref={detailPane}
                  api={api}
                  changesetId={changesetId}
                  scopes={scopes}
                  announce={setStatus}
                  fail={setFailure}
                />
              )}
            </div>
          </div>
        </div>

        <LiveRegion kind="status">{status}</LiveRegion>
        <LiveRegion kind="alert">{failure}</LiveRegion>
      </div>
    </Page>
  );
}
