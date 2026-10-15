import {
  type MutableRefObject,
  type RefObject,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import type { Api, RecordKind } from "../../api/client";
import { t } from "../../i18n";
import { useFocusAfterCommit } from "../../ui/focus";
import { LiveRegion } from "../../ui/LiveRegion";
import { SidePanel } from "../../ui/SidePanel";
import { StandaloneLink } from "../../ui/StandaloneLink";
import { TypeIcon } from "../../ui/TypeIcon";
import { oneLineWording } from "../decisionWording";
import { EvidenceList } from "../EvidenceList";
import type { Work } from "../useWork";
import type { WorkItem } from "../workItem";
import { recordHref } from "../workLinks";
import { canEdit } from "../workPermissions";
import type { ChoiceSources } from "./choices";
import type { FieldSpec, RecordDescriptor, RecordLookup } from "./descriptor";
import { descriptorOf } from "./descriptors";
import { FieldEditor, type FieldEnv } from "./FieldEditor";
import { RecordActions } from "./RecordActions";
import { RecordCreate } from "./RecordCreate";
import { RecordHints } from "./RecordHints";
import styles from "./RecordPanel.module.css";

/** What the panel shows: a record that the address names, or the form of a new record. */
export type PanelMode = { mode: "open"; readableId: string } | { mode: "create"; kind: RecordKind };

export interface RecordPanelProps {
  /** For the pickers that search on the server. */
  api: Api;
  work: Work;
  eventId: string;
  route: PanelMode;
  /** The member who is signed in: the owner that a new record starts with, if they can own one. */
  me: string;
  /** True if the member may create a record. Otherwise the create state says why it is empty. */
  mayCreate: boolean;
  /** Focus goes to the title of the record that the create state made. */
  focusTitle: boolean;
  /** Shows the record that the create state made. */
  onCreated: (readableId: string) => void;
  /** False in a closed event: then no field has a control. */
  writable: boolean;
  /** The time zone of the event, for the times of the evidence. */
  timeZone: string;
  /** The civil date of today. */
  today: string;
  /** The records before and after, in the order of the view. Without one, the button is absent. */
  previous: string | undefined;
  next: string | undefined;
  /** The place of the record among them, counting from one. */
  place: { index: number; total: number } | undefined;
  /** Shows another record in place of this one: the step buttons. */
  onOpen: (readableId: string) => void;
  /** Shows the record that a link of the panel names. The member chose it, so Back returns. */
  onFollow: (readableId: string) => void;
  onClose: () => void;
  /** Where focus goes on close when the element that opened the panel is gone. */
  returnFocusTo: () => HTMLElement | null | undefined;
}

/** The readable IDs of the records that the page has loaded. */
function lookupOf(work: Work): RecordLookup {
  return {
    readableId: (kind, id) =>
      work.items.find((candidate) => candidate.kind === kind && candidate.id === id)?.readableId,
  };
}

function sourcesOf(work: Work, api: Api): ChoiceSources {
  return {
    api,
    records: lookupOf(work),
    directory: work.directory.kind === "loaded" ? work.directory.directory : undefined,
    milestones: work.kinds.milestone.kind === "loaded" ? work.kinds.milestone.records : undefined,
  };
}

/** The head: what the record is. Its status is the first property, so the head does not repeat it. */
function Head({ item }: { item: WorkItem }) {
  return (
    <div className={styles.heading}>
      <TypeIcon kind={item.kind} size={16} decorative />
      <span className={styles.recordId}>{item.readableId}</span>
      <span aria-hidden="true">·</span>
      <span>{t(`record-kind-${item.kind}`)}</span>
    </div>
  );
}

function Property({ spec, item, env }: { spec: FieldSpec; item: WorkItem; env: FieldEnv }) {
  const Icon = spec.icon;
  return (
    <>
      <dt className={styles.label}>
        {Icon && <Icon size={14} stroke={1.5} aria-hidden="true" />}
        {t(spec.labelKey)}
      </dt>
      <dd className={styles.field}>
        <FieldEditor spec={spec} item={item} env={env} />
      </dd>
    </>
  );
}

/** A long text under the properties: its label is the heading of a section. */
function TextSection({ spec, item, env }: { spec: FieldSpec; item: WorkItem; env: FieldEnv }) {
  const headingId = useId();
  return (
    <section className={styles.section} aria-labelledby={headingId}>
      <h3 id={headingId} className={styles.sectionHeading}>
        {t(spec.labelKey)}
      </h3>
      <FieldEditor spec={spec} item={item} env={env} />
    </section>
  );
}

/** The title is the button of an editable title, else the heading itself. */
const titleOf = (heading: RefObject<HTMLHeadingElement | null>) =>
  heading.current?.querySelector<HTMLElement>("button") ?? heading.current;

function Body({
  item,
  descriptor,
  env,
  lookup,
  timeZone,
  focusTitle,
  handoff,
  notice,
  ref,
  heading,
}: {
  ref: RefObject<HTMLDivElement | null>;
  /** The heading element, which the panel also needs to move focus to the title. */
  heading: RefObject<HTMLHeadingElement | null>;
  /** What the member must know about the old state that the record shows, if there is such a thing. */
  notice: string | undefined;
  item: WorkItem;
  descriptor: RecordDescriptor;
  env: FieldEnv;
  lookup: RecordLookup;
  timeZone: string;
  focusTitle: boolean;
  /**
   * True while focus left with a body that held it, so that the next body puts it on its title.
   * The member followed a link or stepped back to a record: the element they used is gone.
   */
  handoff: MutableRefObject<boolean>;
}) {
  const sourcesId = useId();
  const titleId = useId();
  const { title, description } = descriptor;
  // A layout effect, because its cleanup runs before React removes the body: focus is still in it.
  useLayoutEffect(() => {
    const body = ref.current;
    return () => {
      if (body?.contains(document.activeElement)) {
        handoff.current = true;
      }
    };
  }, [handoff, ref]);
  // biome-ignore lint/correctness/useExhaustiveDependencies: once, when the body appears
  useEffect(() => {
    if (handoff.current) {
      handoff.current = false;
      titleOf(heading)?.focus();
    }
  }, []);

  return (
    <div ref={ref} className={styles.body}>
      {/* The field is the heading: it names itself by the title while the title is on screen. */}
      <FieldEditor
        spec={title}
        item={item}
        env={env}
        heading
        valueId={titleId}
        focusOnMount={focusTitle}
        headingRef={heading}
      />
      <dl className={styles.props}>
        {descriptor.properties
          .filter((spec) => spec.when?.(item) ?? true)
          .filter((spec) => !spec.hideEmpty || spec.read(item) !== null)
          .map((spec) => (
            <Property key={spec.id} spec={spec} item={item} env={env} />
          ))}
      </dl>
      {notice && <p className={styles.error}>{notice}</p>}
      <RecordHints hints={descriptor.hints?.(item, lookup) ?? []} />
      {!item.redacted &&
        [description, ...(descriptor.texts ?? [])].map(
          (spec) => spec && <TextSection key={spec.id} spec={spec} item={item} env={env} />,
        )}
      {item.record.evidence.length > 0 && (
        <section className={`${styles.section} ${styles.sources}`} aria-labelledby={sourcesId}>
          <h3 id={sourcesId} className={styles.sectionHeading}>
            {t("record-sources")}
          </h3>
          <EvidenceList
            evidence={item.record.evidence}
            timeZone={timeZone}
            versionText={(version) => t(descriptor.evidenceVersionKey, { version })}
          />
        </section>
      )}
    </div>
  );
}

/**
 * The side panel of a record on the „Arbeit“ page (ADR 0082): the head with the kind, the ID and the
 * status, the title, the properties, the description and the sources. In the create state it holds
 * the form of a new record. It knows no kind: what a kind shows, changes and creates is in its
 * descriptor.
 */
export default function RecordPanel({
  api,
  work,
  eventId,
  route,
  me,
  mayCreate,
  focusTitle,
  onCreated,
  writable,
  timeZone,
  today,
  previous,
  next,
  place,
  onOpen,
  onFollow,
  onClose,
  returnFocusTo,
}: RecordPanelProps) {
  const handoff = useRef(false);
  const [status, setStatus] = useState<string>();
  const [alert, setAlert] = useState<string>();
  // An action that ran, but the record shows the old state: no action is offered for that state.
  const [stale, setStale] = useState<{ id: string; version: number; message: string }>();
  const body = useRef<HTMLDivElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  const frame = {
    onClose,
    position: place && t("side-panel-position", place),
    returnFocusTo,
    ...(previous && { onPrevious: () => onOpen(previous) }),
    ...(next && { onNext: () => onOpen(next) }),
  };
  // One region for every state, outside the switch: it stays mounted when the create state turns
  // into the record that it made, so that the text it gains then is spoken.
  const live = (
    <LiveRegion kind="status" visuallyHidden>
      {status}
    </LiveRegion>
  );

  if (route.mode === "create") {
    const assignees = work.directory.kind === "loaded" ? work.directory.directory.assignees : [];
    // Without the people the form cannot say who may own a record, which is not a closed event.
    const loaded = work.directory.kind === "loaded";
    const descriptor = descriptorOf(route.kind);
    const create = descriptor?.create;
    // A kind that only managers create (a decision) says so to a member who may create the others.
    const isManager = work.directory.kind === "loaded" && work.directory.directory.isManager;
    const managersOnly = mayCreate && create?.managersOnly === true && !isManager;
    const forbidden = !loaded
      ? "record-create-unavailable"
      : managersOnly
        ? "record-create-managers-only"
        : "record-create-forbidden";
    return (
      <SidePanel title={create ? t(create.titleKey) : ""} {...frame}>
        {descriptor && create && mayCreate && !managersOnly ? (
          <RecordCreate
            descriptor={descriptor}
            api={api}
            commands={work.commands}
            sources={sourcesOf(work, api)}
            owner={assignees.some((member) => member.id === me) ? me : undefined}
            onCreated={onCreated}
            recover={(id) => work.locate(route.kind, id)}
            onCancel={onClose}
            announce={setStatus}
          />
        ) : (
          <p>{t(forbidden)}</p>
        )}
        {live}
      </SidePanel>
    );
  }

  const { readableId } = route;
  const item = work.items.find((candidate) => candidate.readableId === readableId);
  const descriptor = item && descriptorOf(item.kind);
  if (item && !descriptor) {
    // The record exists, but its kind has no panel yet: send the member to the old page.
    const href = recordHref(eventId, item);
    return (
      <SidePanel title={t("record-no-panel-title")} {...frame}>
        <div className={styles.notFound}>
          <h2 className={styles.title}>{t("record-no-panel-title")}</h2>
          <p>{t("record-no-panel-text", { id: readableId })}</p>
          {href && <StandaloneLink to={href}>{t("record-no-panel-open")}</StandaloneLink>}
        </div>
        {live}
      </SidePanel>
    );
  }
  if (!item || !descriptor) {
    return (
      <SidePanel title={t("record-not-found-title")} {...frame}>
        <div className={styles.notFound}>
          <h2 className={styles.title}>{t("record-not-found-title")}</h2>
          <p>{t("record-not-found-text", { id: readableId })}</p>
        </div>
        {live}
      </SidePanel>
    );
  }
  const env: FieldEnv = {
    commands: work.commands,
    refresh: work.refresh,
    announce: (text, urgency) => (urgency === "alert" ? setAlert(text) : setStatus(text)),
    sources: sourcesOf(work, api),
    today,
    editable: canEdit(item, writable),
    writable,
    open: onFollow,
  };
  const notice =
    stale?.id === item.id && stale.version === item.version ? stale.message : undefined;
  const lookup = lookupOf(work);
  // A decision has a long wording: the name of the region has its first line.
  const name = item.redacted ? item.readableId : `${item.readableId} ${oneLineWording(item.title)}`;
  return (
    <SidePanel
      title={name}
      heading={<Head item={item} />}
      more={
        descriptor.actions &&
        !item.redacted && (
          <RecordActions
            actions={descriptor.actions}
            item={item}
            env={env}
            paused={notice !== undefined}
            onDone={(message, urgency) => {
              env.announce(message, urgency);
              if (urgency === "alert") {
                setStale({ id: item.id, version: item.version, message });
              }
              // The menu leaves with the state that the record has now; the title stays.
              focusAfterCommit(() => titleOf(heading));
            }}
          />
        )
      }
      {...frame}
    >
      <Body
        ref={body}
        heading={heading}
        notice={notice}
        key={item.id}
        item={item}
        descriptor={descriptor}
        env={env}
        lookup={lookup}
        timeZone={timeZone}
        focusTitle={focusTitle}
        handoff={handoff}
      />
      {live}
      {/* The failure of a save: at once, and only where a field can fail. */}
      <LiveRegion kind="alert" visuallyHidden>
        {alert}
      </LiveRegion>
    </SidePanel>
  );
}
