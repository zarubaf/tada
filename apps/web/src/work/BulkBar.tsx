import {
  IconAlertTriangle,
  IconLayersSubtract,
  IconProgressCheck,
  IconUser,
} from "@tabler/icons-react";
import { type Ref, useId, useLayoutEffect, useMemo, useRef, useState } from "react";
import { t } from "../i18n";
import { Avatar } from "../ui/Avatar";
import { Banner } from "../ui/Banner";
import { Button } from "../ui/Button";
import { PickerButton } from "../ui/PickerButton";
import type { SearchOption } from "../ui/SearchList";
import { StatusIcon } from "../ui/StatusIcon";
import { Toolbar } from "../ui/Toolbar";
import styles from "./BulkBar.module.css";
import { type BulkChange, statusChoice, supports } from "./bulkChange";
import { statusPresentationOfText } from "./recordStatus";
import type { BulkChanger, BulkFailure } from "./useBulkChange";
import type { Work } from "./useWork";
import { itemKey, type WorkSelection } from "./useWorkSelection";

const NO_AREA = "none";

export interface BulkBarProps {
  selection: WorkSelection;
  /** The change that the page runs: it goes on when the bar leaves. */
  bulk: BulkChanger;
  work: Pick<Work, "commands" | "directory">;
  /** Below 40rem the bar sits above the bottom bar and the lists open as sheets. */
  narrow: boolean;
  /** Says a result in the polite region of the page. Without text it empties the region. */
  announce: (text: string | undefined) => void;
  /** The banner of the failures, for the focus that the page moves to it. */
  bannerRef: Ref<HTMLDivElement>;
  /** The dock, for the page to tell whether focus is in the bar. */
  dockRef: Ref<HTMLDivElement>;
}

/** The short word for a failure; a reason without one shows the message of the server. */
function reasonText(failure: BulkFailure): string {
  switch (failure.reason) {
    case "conflict":
    case "forbidden":
    case "not-found":
    case "validation":
      return t(`work-bulk-reason-${failure.reason}`);
    case "other":
      return failure.message;
  }
}

const capitalized = (text: string) => text.charAt(0).toLocaleUpperCase("de-CH") + text.slice(1);

/**
 * The bar for the selected records (doc/design/components.md, „Bulk bar“): „Status“,
 * „Verantwortlich“ and „Bereich“ change all selected records, „Auswahl aufheben“ ends the
 * selection. It sits at the bottom of the work area, above the bottom bar on a phone. A control
 * that cannot work for this selection refuses the press, stays focusable and says why in its
 * description.
 */
export function BulkBar({
  selection,
  bulk,
  work,
  narrow,
  announce,
  bannerRef,
  dockRef,
}: BulkBarProps) {
  const { commands, directory } = work;
  const { failures, progress, reloading } = bulk.state;
  const items = selection.items;
  const status = useMemo(() => statusChoice(commands, items), [commands, items]);
  const loaded = directory.kind === "loaded" ? directory.directory : undefined;
  const directoryReason = loaded ? undefined : t("work-bulk-reason-directory");

  const statusReason = status.allowed ? undefined : t(`work-bulk-reason-status-${status.reason}`);
  const statusOptions: SearchOption[] = status.allowed
    ? status.statuses.map((value) => {
        const { glyph, tone, labelKey } = statusPresentationOfText(status.kind, value);
        return {
          id: value,
          label: capitalized(t(labelKey)),
          leading: <StatusIcon glyph={glyph} tone={tone} size={14} />,
        };
      })
    : [];
  const ownerOptions: SearchOption[] = (loaded?.assignees ?? []).map((member) => ({
    id: member.id,
    label: member.label,
    leading: <Avatar name={member.label} id={member.id} decorative />,
  }));
  const areaOptions: SearchOption[] = [
    { id: NO_AREA, label: t("work-bulk-no-area") },
    ...(loaded?.workstreams ?? [])
      .filter((stream) => stream.status === "active")
      .map((stream) => ({ id: stream.id, label: stream.name })),
  ];

  const apply = (change: BulkChange) => {
    announce(undefined);
    if (bulk.run(items, change) === "unchanged") {
      announce(t("work-bulk-nothing"));
    }
  };

  // No new run while one runs or its kinds load again: it would use versions that are about to change.
  // Every control then says so as its reason, so that the refusal is never silent.
  const busy = progress !== undefined || reloading;
  const busyReason = busy ? t("work-bulk-busy") : undefined;
  const unsupported = (field: "owner" | "area") =>
    supports(commands, items, field) ? undefined : t("work-bulk-reason-unsupported");
  const retryReason = useId();
  const clearReason = useId();
  // Only the failures of selected records are shown and tried again: the others left the sight.
  const selectedKeys = useMemo(() => new Set(items.map(itemKey)), [items]);
  const shownFailures = failures.filter((failure) => selectedKeys.has(failure.key));

  // A list that scrolls is a tab stop, so that the keyboard can read all of it.
  const list = useRef<HTMLUListElement>(null);
  const [scrolls, setScrolls] = useState(false);
  // biome-ignore lint/correctness/useExhaustiveDependencies: the size of the list is the trigger
  useLayoutEffect(() => {
    const element = list.current;
    setScrolls(element !== null && element.scrollHeight > element.clientHeight);
  }, [shownFailures.length, narrow]);

  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: Escape is a shortcut for the controls inside
    <div
      ref={dockRef}
      className={styles.dock}
      aria-busy={busy}
      onKeyDown={(event) => {
        if (event.key === "Escape" && !event.defaultPrevented) {
          selection.clear();
        }
      }}
    >
      {shownFailures.length > 0 && (
        <Banner ref={bannerRef} icon={IconAlertTriangle}>
          <p>
            <strong>
              {t("work-bulk-failed-title", {
                count: shownFailures.length,
                total: bulk.state.total,
              })}
            </strong>
          </p>
          <ul
            ref={list}
            className={styles.failures}
            aria-label={t("work-bulk-failures")}
            tabIndex={scrolls ? 0 : undefined}
          >
            {shownFailures.map((failure) => (
              <li key={failure.key}>
                {failure.readableId}: {reasonText(failure)}
              </li>
            ))}
          </ul>
          <Button
            isRefused={busy}
            aria-describedby={busy ? retryReason : undefined}
            onPress={() => {
              announce(undefined);
              bulk.retry(selectedKeys);
            }}
          >
            {t("retry")}
          </Button>
          {busy && (
            <span id={retryReason} hidden>
              {busyReason}
            </span>
          )}
        </Banner>
      )}
      <Toolbar label={t("work-bulk-label")} className={styles.bar}>
        <span className={styles.count}>
          {progress
            ? t("work-bulk-progress", { done: progress.done, total: progress.total })
            : t("work-bulk-count", { count: items.length })}
        </span>
        <div className={styles.actions}>
          <PickerButton
            label={t("work-bulk-status")}
            icon={IconProgressCheck}
            options={statusOptions}
            disabledReason={busyReason ?? statusReason}
            searchable={false}
            narrow={narrow}
            onChoose={(id) => apply({ field: "status", status: id })}
          />
          <PickerButton
            label={t("work-bulk-owner")}
            icon={IconUser}
            options={ownerOptions}
            disabledReason={busyReason ?? directoryReason ?? unsupported("owner")}
            narrow={narrow}
            onChoose={(id) => apply({ field: "owner", userId: id })}
          />
          <PickerButton
            label={t("work-bulk-area")}
            icon={IconLayersSubtract}
            options={areaOptions}
            disabledReason={busyReason ?? directoryReason ?? unsupported("area")}
            narrow={narrow}
            onChoose={(id) => apply({ field: "area", workstreamId: id === NO_AREA ? null : id })}
          />
        </div>
        <Button
          variant="quiet"
          isRefused={selection.locked}
          aria-describedby={selection.locked ? clearReason : undefined}
          onPress={selection.clear}
        >
          {t("work-bulk-clear")}
        </Button>
        {selection.locked && (
          <span id={clearReason} hidden>
            {t("work-bulk-busy")}
          </span>
        )}
      </Toolbar>
    </div>
  );
}
