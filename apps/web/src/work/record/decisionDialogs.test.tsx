import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { createApi } from "../../api/client";
import { json } from "../../test/fakeWorkServer";
import * as r from "../../test/workRecords";
import { toWorkItem } from "../workItem";
import { SupersedeDecisionDialog } from "./decisionDialogs";
import type { FieldEnv } from "./FieldEditor";

const party = (id: string, localId: string, name: string, redacted: boolean) => ({
  kind: "person",
  id,
  local_id: localId,
  ...(redacted ? { redacted } : { name, redacted }),
});

describe("SupersedeDecisionDialog", () => {
  it("names a removed party by the redaction text, not by its ID, and a known one by its name", () => {
    const item = toWorkItem(
      "decision",
      r.decision("DEC-001", {
        decided_by: [
          party("p1", "PER-001", "Cäcilia Probst", false),
          party("p2", "PER-002", "", true),
        ],
      }) as never,
      () => "",
    );
    const api = createApi((async () => json(200, { items: [] })) as unknown as typeof fetch);
    const env = {
      commands: {},
      refresh: vi.fn(async () => true),
      sources: { api, directory: undefined, milestones: undefined },
    } as unknown as FieldEnv;
    render(
      <SupersedeDecisionDialog
        item={item}
        env={env}
        onDone={vi.fn()}
        onConflict={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    expect(screen.getByText("Cäcilia Probst (PER-001)")).toBeVisible();
    expect(screen.getByText("entfernt")).toBeVisible();
    expect(screen.queryByText("PER-002")).toBeNull();
  });
});
