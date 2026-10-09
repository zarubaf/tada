// The calls of persons and institutions behind one contract, so the page and the form do not
// branch on the kind of the party (ADR 0069).
import type { Api, Institution, Person, Problem } from "../api/client";

export type PartyKind = "person" | "institution";
export type Party = Person | Institution;

/** The fields that a form sends. `null` clears an optional field in a change. */
export interface PartyInput {
  name: string;
  kind?: string;
  email: string | null;
  phone: string | null;
}

export interface Page {
  items: Party[];
  nextCursor: string | undefined;
}

type Answer<T> = Promise<{ data?: T; error?: Problem | undefined; response: Response }>;

export interface PartyApi {
  list(query: { q?: string; cursor?: string }): Answer<Page>;
  create(input: PartyInput): Answer<Party>;
  change(record: Party, input: PartyInput): Answer<Party>;
}

/** An absent optional field is left out of a new record. */
function optional(input: PartyInput) {
  return {
    name: input.name,
    ...(input.email === null ? {} : { email: input.email }),
    ...(input.phone === null ? {} : { phone: input.phone }),
  };
}

function page(data: { items: Party[]; next_cursor?: string | null }): Page {
  return { items: data.items, nextCursor: data.next_cursor ?? undefined };
}

export function partyApi(api: Api, kind: PartyKind): PartyApi {
  return kind === "person" ? personApi(api) : institutionApi(api);
}

function personApi(api: Api): PartyApi {
  return {
    async list(query) {
      const { data, error, response } = await api.GET("/api/v1/persons", { params: { query } });
      return { ...(data && { data: page(data) }), error, response };
    },
    create: (input) => api.POST("/api/v1/persons", { body: optional(input) }),
    change: (record, input) =>
      api.PATCH("/api/v1/persons/{person_id}", {
        params: { path: { person_id: record.id } },
        body: { ...input, expected_version: record.version },
      }),
  };
}

function institutionApi(api: Api): PartyApi {
  return {
    async list(query) {
      const { data, error, response } = await api.GET("/api/v1/institutions", {
        params: { query },
      });
      return { ...(data && { data: page(data) }), error, response };
    },
    create: (input) =>
      api.POST("/api/v1/institutions", {
        body: { ...optional(input), kind: input.kind ?? "other" },
      }),
    change: (record, input) =>
      api.PATCH("/api/v1/institutions/{institution_id}", {
        params: { path: { institution_id: record.id } },
        body: { ...input, expected_version: record.version },
      }),
  };
}
