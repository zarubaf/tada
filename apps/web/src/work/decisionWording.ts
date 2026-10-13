// The shortened wording of a decision for a table row: the first lines, cut at a length. The
// wording itself never changes (ADR 0080); the row only decides how much of it shows.

const MAX_LINES = 3;
const MAX_CHARACTERS = 160;

interface ShortWording {
  text: string;
  /** True if the text leaves something out. */
  cut: boolean;
}

export function shortWording(wording: string): ShortWording {
  const lines = wording.split("\n");
  let text = lines.slice(0, MAX_LINES).join("\n");
  let cut = lines.length > MAX_LINES;
  if (text.length > MAX_CHARACTERS) {
    text = text.slice(0, MAX_CHARACTERS).trimEnd();
    cut = true;
  }
  return { text: cut ? `${text} …` : text, cut };
}

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
