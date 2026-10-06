# 0040. AI intake through MCP first

- Status: Proposed
- Date: 2026-10-06

## Context

In Slice 1, the product owner wants an LLM agent to structure the event data from free text: for example "Dübendorf airshow, public, two days, date open".
The product owner already uses Claude and Codex.
An intake inside tada needs the model adapter (ADR 0010) and the data protection ADR before real data goes to a model provider.
ADR 0039 defines personal API tokens with the scopes `read` and `propose`, and treats each token caller as AI.
`rmcp` is the official Rust SDK for the Model Context Protocol.

## Decision

Channel:

- In Slice 1, members use their own MCP client, for example Claude Code or Codex, to enter and query data.
- The intake inside tada (web text field and Telegram) follows in Slice 2, through the model adapter.
- Both ways use the same `app` commands. Only the driving adapter differs.

Server:

- A new `mcp` crate is a driving adapter, like `api` and `telegram`. It uses `rmcp` with the Streamable HTTP transport.
- `serve` provides it at `/mcp`.
- Authentication is a personal API token (ADR 0039) in the `Authorization: Bearer` header. Claude Code and Codex support this header for HTTP servers.
- OAuth for clients that need it, for example Claude Desktop connectors, needs a new ADR.

Tools:

- Read tools: list the member's events, get the event schema, get the event profile with its status values (accepted, assumption, unknown, proposal), search sources.
- Proposal tools: propose a new event, a fact, an assumption, an explicit unknown, an open question, or a document draft.
- No tool accepts, rejects or deletes. The member accepts proposals in the Review Inbox of the web client.
- The tool input and output schemas come from the same Rust types as the `app` commands, through `schemars`.

Evidence:

- Each proposal call contains the member's own words as `source_text` and the passage that supports each value.
- tada stores the text as a source version with the channel `api-token`. Each proposal links to its passage.
- A proposal without a supporting passage is rejected with a problem code (ADR 0037).

Data protection:

- With MCP, the member's own AI client processes the text, under the member's account. tada sends nothing to a model provider.
- The privacy notice states that members can connect their own AI clients, and which data the read tools give them.

## Consequences

- Slice 1 needs no model adapter in tada. The German concept and the enquiry draft come from the member's agent as document draft proposals.
- AI cost in Slice 1 is zero for the operator.
- API tokens move from "later" into Slice 1.
- Members without an MCP client can use only the forms of the web client until Slice 2.

## Alternatives

- Intake inside tada first: needs the model adapter, quotas and the data protection ADR before the first demonstration.
- A REST API for agents without MCP: each agent would need its own integration code.
- MCP with write tools: an AI could change accepted state, against ADR 0010.
