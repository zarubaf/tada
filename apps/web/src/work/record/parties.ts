// A party is a person or an institution. A form holds it as one ID, `person:{uuid}` or
// `institution:{uuid}`, which is how the search of the server (ADR 0069) names a result; the
// request names it as a kind and a UUID. The party control of a commitment and the people field of a
// decision share this.
import type { CreateDecisionRequest } from "../../api/client";
import { type FieldValue, peopleIds } from "./descriptor";

type Party = CreateDecisionRequest["decided_by"][number];

/** The party that an ID of a form names, as the request names it. */
export function partyOf(id: string): Party {
  const [kind, uuid = ""] = id.split(":");
  return { kind: kind === "person" ? "person" : "institution", id: uuid };
}

/** The parties of a `people` value as the request names them. */
export const partiesOf = (value: FieldValue): Party[] => peopleIds(value).map(partyOf);

/** The ID of a party as a form holds it. */
export const partyId = (party: Party) => `${party.kind}:${party.id}`;
