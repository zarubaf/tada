// The calls of the data-protection section of a person (ADR 0073). Only an owner may make them.
import type { Api, Problem } from "../api/client";
import type { components } from "../api/schema.gen";

export type Mention = components["schemas"]["PersonMention"];

/** The kinds of mention that a redaction or a dispute note can target. Others edit normally. */
const TARGET_KINDS = ["source-item", "fact-version", "document-version", "proposal"];

/** The mention is selectable for a redaction or a dispute note. A redacted mention is not. */
export function isTarget(mention: Mention): boolean {
  return TARGET_KINDS.includes(mention.kind) && !mention.redacted;
}

/** The key that identifies a mention in a selection. */
export function mentionKey(mention: Mention): string {
  return `${mention.kind}:${mention.id}`;
}

/** The address of the export. A plain link: the server sends it as an attachment. */
export function exportUrl(personId: string): string {
  return `/api/v1/persons/${encodeURIComponent(personId)}/export`;
}

type Answer = { error?: Problem | undefined; response: Response };

export interface PersonDataApi {
  mentions(personId: string): Promise<Answer & { data?: { items: Mention[] } | undefined }>;
  redact(targets: Mention[]): Promise<Answer>;
  dispute(target: Mention, text: string): Promise<Answer>;
}

export function personDataApi(api: Api): PersonDataApi {
  return {
    mentions: (personId) =>
      api.GET("/api/v1/persons/{person_id}/mentions", {
        params: { path: { person_id: personId } },
      }),
    redact: (targets) =>
      api.POST("/api/v1/redactions", {
        body: { targets: targets.map(({ kind, id }) => ({ kind, id })), reason: "request" },
      }),
    dispute: (target, text) =>
      api.POST("/api/v1/dispute-notes", {
        body: { target_kind: target.kind, target_id: target.id, text },
      }),
  };
}
