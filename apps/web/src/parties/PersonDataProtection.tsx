import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import personDataMessages from "../../../../locales/de-CH/person-data.ftl?raw";
import { type Api, problemMessage } from "../api/client";
import { addMessages, hasMessage, t } from "../i18n";
import { SignInAgain } from "../session/SignInAgain";
import { Button } from "../ui/Button";
import { Checkbox } from "../ui/Checkbox";
import { ConfirmDialog } from "../ui/ConfirmDialog";
import { useFocusAfterCommit } from "../ui/focus";
import { LiveRegion } from "../ui/LiveRegion";
import { Redacted } from "../ui/Redacted";
import { StandaloneLink } from "../ui/StandaloneLink";
import { DisputeDialog } from "./DisputeDialog";
import styles from "./PersonDataProtection.module.css";
import { exportFileName, isTarget, type Mention, mentionKey, personDataApi } from "./personDataApi";

/** The page of the records that a mention of another kind belongs to, for the normal edit. */
addMessages(personDataMessages);

const RECORD_PAGES: Record<string, string> = {
  action: "actions",
  commitment: "commitments",
  promisor: "commitments",
  workstream: "workstreams",
};

function recordPath(mention: Mention): string | undefined {
  if (!mention.event_id) {
    return undefined;
  }
  const page = RECORD_PAGES[mention.kind];
  const event = `/events/${encodeURIComponent(mention.event_id)}`;
  return page ? `${event}/${page}` : event;
}

function kindName(mention: Mention): string {
  const id = `mention-kind-${mention.kind}`;
  return hasMessage(id) ? t(id) : mention.kind;
}

function MentionRow({
  mention,
  selected,
  onSelect,
}: {
  mention: Mention;
  selected: boolean;
  onSelect: (on: boolean) => void;
}) {
  const text = mention.excerpt ?? "";
  const label = `${kindName(mention)}: ${text}`;
  const path = recordPath(mention);
  return (
    <li
      className={
        mention.redacted || !isTarget(mention)
          ? `${styles.mention} ${styles.plain}`
          : styles.mention
      }
    >
      {mention.redacted ? (
        <>
          <span className={styles.kind}>{kindName(mention)}:</span>
          <Redacted />
        </>
      ) : isTarget(mention) ? (
        <Checkbox label={label} isSelected={selected} onChange={onSelect} />
      ) : path ? (
        <StandaloneLink to={path}>{label}</StandaloneLink>
      ) : (
        <span>{label}</span>
      )}
      {mention.disputed && <span className={styles.flag}>{t("person-data-disputed")}</span>}
    </li>
  );
}

/**
 * „Datenschutz“ of a person, for an owner (ADR 0073): the export, the mentions of the person in
 * the texts of the organization, and the removal or the objection for the evidence among them.
 */
export default function PersonDataProtection({ api, personId }: { api: Api; personId: string }) {
  const data = useMemo(() => personDataApi(api), [api]);
  const [mentions, setMentions] = useState<Mention[]>();
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [confirming, setConfirming] = useState(false);
  const [disputing, setDisputing] = useState(false);
  const [busy, setBusy] = useState(false);
  const [exporting, setExporting] = useState(false);
  // The export and the removal need a recent sign-in (ADR 0070): the page then offers a new one.
  const [signInAgain, setSignInAgain] = useState(false);
  const [failure, setFailure] = useState<string>();
  const [done, setDone] = useState<string>();
  const heading = useRef<HTMLHeadingElement>(null);
  const focusAfterCommit = useFocusAfterCommit();

  const load = useCallback(async () => {
    try {
      const result = await data.mentions(personId);
      if (result.data) {
        setMentions(result.data.items);
      } else {
        setFailure(problemMessage(result.error));
      }
    } catch {
      setFailure(problemMessage(undefined));
    }
  }, [data, personId]);
  useEffect(() => {
    void load();
  }, [load]);

  const chosen = (mentions ?? []).filter((mention) => selected.has(mentionKey(mention)));

  const toggle = (mention: Mention, on: boolean) => {
    const next = new Set(selected);
    if (on) {
      next.add(mentionKey(mention));
    } else {
      next.delete(mentionKey(mention));
    }
    setSelected(next);
  };

  /** Shows the message of a refused call, and the way to a new sign-in if the server asks for it. */
  const refused = (error: Parameters<typeof problemMessage>[0]) => {
    setSignInAgain(error?.code === "recent-sign-in-required");
    setFailure(problemMessage(error));
  };

  const exportData = async () => {
    setExporting(true);
    setFailure(undefined);
    try {
      const result = await data.exportFile(personId);
      if (result.data && result.response.ok) {
        const url = URL.createObjectURL(result.data);
        const link = document.createElement("a");
        link.href = url;
        link.download = exportFileName(result.response);
        link.click();
        // Older WebKit versions stop the download if the URL goes in the same task as the click.
        setTimeout(() => URL.revokeObjectURL(url), 0);
      } else {
        refused(result.error);
      }
    } catch {
      setFailure(problemMessage(undefined));
    }
    setExporting(false);
  };

  const redact = async () => {
    setBusy(true);
    setFailure(undefined);
    try {
      const result = await data.redact(chosen);
      if (result.error || !result.response.ok) {
        refused(result.error);
      } else {
        setDone(t("person-data-redacted", { count: chosen.length }));
        setSelected(new Set());
        focusAfterCommit(() => heading.current);
        await load();
      }
    } catch {
      setFailure(problemMessage(undefined));
    }
    setConfirming(false);
    setBusy(false);
  };

  const dispute = async (text: string): Promise<string | undefined> => {
    const target = chosen[0];
    if (!target) {
      return undefined;
    }
    try {
      const result = await data.dispute(target, text);
      if (result.error || !result.response.ok) {
        if (result.error?.code === "validation-failed") {
          return t("person-data-dispute-error");
        }
        return result.error?.code === "invalid-transition"
          ? t("person-data-dispute-redacted")
          : problemMessage(result.error);
      }
    } catch {
      return problemMessage(undefined);
    }
    setDisputing(false);
    setSelected(new Set());
    setDone(t("person-data-disputed-done"));
    focusAfterCommit(() => heading.current);
    await load();
    return undefined;
  };

  return (
    <section className={styles.section} aria-labelledby="person-data-title">
      <h2 id="person-data-title" className={styles.heading}>
        {t("person-data-title")}
      </h2>
      <LiveRegion kind="alert">{failure}</LiveRegion>
      <LiveRegion kind="status">{done}</LiveRegion>
      <p>
        <Button isPending={exporting} onPress={() => void exportData()}>
          {t("person-data-export")}
        </Button>
      </p>
      {signInAgain && <SignInAgain />}

      <h3 ref={heading} tabIndex={-1} className={styles.subheading}>
        {t("person-data-mentions")}
      </h3>
      {mentions === undefined ? (
        !failure && <p>{t("person-data-mentions-loading")}</p>
      ) : mentions.length === 0 ? (
        <p>{t("person-data-mentions-none")}</p>
      ) : (
        <>
          <p className={styles.hint}>{t("person-data-hint")}</p>
          <ul className={styles.list}>
            {mentions.map((mention) => (
              <MentionRow
                key={mentionKey(mention)}
                mention={mention}
                selected={selected.has(mentionKey(mention))}
                onSelect={(on) => toggle(mention, on)}
              />
            ))}
          </ul>
          <div className={styles.actions}>
            <Button
              variant="danger"
              isDisabled={chosen.length === 0}
              onPress={() => setConfirming(true)}
            >
              {t("person-data-redact")}
            </Button>
            <Button isDisabled={chosen.length !== 1} onPress={() => setDisputing(true)}>
              {t("person-data-dispute")}
            </Button>
          </div>
        </>
      )}

      <ConfirmDialog
        isOpen={confirming}
        title={t("person-data-redact-title")}
        text={t("person-data-redact-text", { count: chosen.length })}
        warning={t("person-data-redact-warning")}
        confirmLabel={t("person-data-redact-confirm")}
        cancelLabel={t("work-cancel")}
        isPending={busy}
        onConfirm={() => void redact()}
        onCancel={() => setConfirming(false)}
      />
      {disputing && <DisputeDialog onSubmit={dispute} onCancel={() => setDisputing(false)} />}
    </section>
  );
}
