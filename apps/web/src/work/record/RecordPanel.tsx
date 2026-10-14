import { useId } from "react";
import { t } from "../../i18n";
import { RecordId } from "../../ui/RecordId";
import { SidePanel } from "../../ui/SidePanel";
import { StatusLabel } from "../../ui/StatusLabel";
import { TypeIcon } from "../../ui/TypeIcon";
import { EvidenceList } from "../EvidenceList";
import { presentationOf } from "../recordStatus";
import type { Work } from "../useWork";
import type { WorkItem } from "../workItem";
import { type ChoiceSources, choicesOf } from "./choices";
import type { FieldSpec, RecordDescriptor } from "./descriptor";
import { descriptorOf } from "./descriptors";
import { FieldView } from "./FieldView";
import styles from "./RecordPanel.module.css";

export interface RecordPanelProps {
  work: Work;
  /** The readable ID that the address names. */
  readableId: string;
  /** The time zone of the event, for the times of the evidence. */
  timeZone: string;
  /** The civil date of today. */
  today: string;
  /** The records before and after, in the order of the view. Without one, the button is absent. */
  previous: string | undefined;
  next: string | undefined;
  /** Shows another record. */
  onOpen: (readableId: string) => void;
  onClose: () => void;
  /** Where focus goes on close when the element that opened the panel is gone. */
  returnFocusTo: () => HTMLElement | null | undefined;
}

function sourcesOf(work: Work): ChoiceSources {
  return {
    directory: work.directory.kind === "loaded" ? work.directory.directory : undefined,
    milestones: work.kinds.milestone.kind === "loaded" ? work.kinds.milestone.records : undefined,
  };
}

function Head({ item }: { item: WorkItem }) {
  const { glyph, tone, labelKey } = presentationOf(item);
  return (
    <div className={styles.heading}>
      <TypeIcon kind={item.kind} size={16} />
      <RecordId>{item.readableId}</RecordId>
      <StatusLabel icon={glyph} tone={tone}>
        {t(labelKey)}
      </StatusLabel>
    </div>
  );
}

function Property({
  spec,
  item,
  sources,
  today,
}: {
  spec: FieldSpec;
  item: WorkItem;
  sources: ChoiceSources;
  today: string;
}) {
  const value = spec.read(item);
  const choices = choicesOf(spec, sources, item, value);
  const Icon = spec.icon;
  return (
    <>
      <dt className={styles.label}>
        {Icon && <Icon size={14} stroke={1.5} aria-hidden="true" />}
        {t(spec.labelKey)}
      </dt>
      <dd className={styles.field}>
        <FieldView spec={spec} value={value} choices={choices} item={item} today={today} />
      </dd>
    </>
  );
}

function Body({
  item,
  descriptor,
  sources,
  timeZone,
  today,
}: {
  item: WorkItem;
  descriptor: RecordDescriptor;
  sources: ChoiceSources;
  timeZone: string;
  today: string;
}) {
  const descriptionId = useId();
  const sourcesId = useId();
  const { title, description } = descriptor;
  return (
    <div className={styles.body}>
      <h2 className={styles.title}>
        <FieldView
          spec={title}
          value={title.read(item)}
          choices={choicesOf(title, sources, item, title.read(item))}
          item={item}
          today={today}
        />
      </h2>
      <dl className={styles.props}>
        {descriptor.properties.map((spec) => (
          <Property key={spec.id} spec={spec} item={item} sources={sources} today={today} />
        ))}
      </dl>
      {description && !item.redacted && (
        <section className={styles.section} aria-labelledby={descriptionId}>
          <h3 id={descriptionId} className={styles.sectionHeading}>
            {t(description.labelKey)}
          </h3>
          <FieldView
            spec={description}
            value={description.read(item)}
            choices={choicesOf(description, sources, item, description.read(item))}
            item={item}
            today={today}
          />
        </section>
      )}
      {item.record.evidence.length > 0 && (
        <section className={styles.section} aria-labelledby={sourcesId}>
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
 * status, the title, the properties, the description and the sources. It knows no kind: what a kind
 * shows and changes is in its descriptor.
 */
export default function RecordPanel({
  work,
  readableId,
  timeZone,
  today,
  previous,
  next,
  onOpen,
  onClose,
  returnFocusTo,
}: RecordPanelProps) {
  const item = work.items.find((candidate) => candidate.readableId === readableId);
  // A kind without a descriptor has no panel yet, so its record counts as not found.
  const descriptor = item && descriptorOf(item.kind);
  const frame = {
    onClose,
    returnFocusTo,
    ...(previous && { onPrevious: () => onOpen(previous) }),
    ...(next && { onNext: () => onOpen(next) }),
  };
  if (!item || !descriptor) {
    return (
      <SidePanel title={t("record-not-found-title")} {...frame}>
        <div className={styles.notFound}>
          <h2 className={styles.title}>{t("record-not-found-title")}</h2>
          <p>{t("record-not-found-text", { id: readableId })}</p>
        </div>
      </SidePanel>
    );
  }
  const name = item.redacted ? item.readableId : `${item.readableId} ${item.title}`;
  return (
    <SidePanel title={name} heading={<Head item={item} />} {...frame}>
      <Body
        key={item.id}
        item={item}
        descriptor={descriptor}
        sources={sourcesOf(work)}
        timeZone={timeZone}
        today={today}
      />
    </SidePanel>
  );
}
