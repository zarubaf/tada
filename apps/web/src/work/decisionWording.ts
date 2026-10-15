// The wording of a decision in one line, for a name in a sentence or a list. The wording itself
// never changes (ADR 0081); a name only decides how much of it shows.

const NAME_CHARACTERS = 80;

/** The wording in one line, for a name in a sentence or a list. */
export function oneLineWording(wording: string): string {
  const line = wording.replace(/\s+/g, " ").trim();
  return line.length > NAME_CHARACTERS ? `${line.slice(0, NAME_CHARACTERS).trimEnd()} …` : line;
}

/** The readable ID and the wording of a decision; the ID alone after a legal redaction. */
export function decisionName(decision: { local_id: string; wording: string; redacted: boolean }) {
  return decision.redacted
    ? decision.local_id
    : `${decision.local_id} ${oneLineWording(decision.wording)}`;
}
