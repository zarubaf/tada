import type { RecordEvidence } from "../api/client";
import { Sheet } from "../ui/Sheet";
import { EvidenceList } from "./EvidenceList";

/** The evidence of a work record in a sheet (see `EvidenceList`). */
export function EvidenceSheet({
  title,
  evidence,
  timeZone,
  versionText,
  onClose,
}: {
  title: string;
  evidence: RecordEvidence[];
  timeZone: string;
  /** The line that names the record version that a passage supports. */
  versionText: (version: number) => string;
  onClose: () => void;
}) {
  return (
    <Sheet title={title} onClose={onClose}>
      <EvidenceList evidence={evidence} timeZone={timeZone} versionText={versionText} />
    </Sheet>
  );
}
