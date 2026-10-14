// What a decision takes in when it is made, in the create state and in „Ersetzen“: the wording and
// the parties, as the form holds them and as the API takes them.
import type { CreateDecisionRequest, Decision } from "../../api/client";
import { t } from "../../i18n";
import { type FieldValue, peopleIds } from "./descriptor";

/** The most characters of a wording. */
const MAX_WORDING = 2000;

/** A control character other than the line break and the tab, which a wording may hold. */
const hasForbiddenCharacter = (text: string) => /[\p{Cc}]/u.test(text.replace(/[\n\t]/g, ""));

/**
 * The message for a wording that the server would refuse, or nothing. The server counts the
 * characters of the trimmed text, not the UTF-16 units.
 */
export function wordingError(wording: string): string | undefined {
  const text = wording.trim();
  if ([...text].length > MAX_WORDING) {
    return t("work-error-wording-too-long");
  }
  return hasForbiddenCharacter(text) ? t("work-error-wording-control-character") : undefined;
}

/** The parties of a `people` value as the request names them. */
export function partiesOf(value: FieldValue): CreateDecisionRequest["decided_by"] {
  return peopleIds(value).map((id) => {
    const [kind, uuid = ""] = id.split(":");
    return { kind: kind === "person" ? "person" : "institution", id: uuid };
  });
}

/** The ID of a party as a `people` field holds it: `person:{uuid}` or `institution:{uuid}`. */
export const partyId = (party: Decision["decided_by"][number]) => `${party.kind}:${party.id}`;
