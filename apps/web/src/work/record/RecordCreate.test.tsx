import { act, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { renderWork } from "../../test/fakeWorkServer";
import type { CommandResult, WorkCommands } from "../workCommands";
import type { CreateSpec, FieldSpec, FieldValue, RecordDescriptor, Saved } from "./descriptor";
import { RecordCreate } from "./RecordCreate";

const user = userEvent.setup({ delay: null });
const SEARCH = { timeout: 3000 };

const field = (id: string, over: Partial<FieldSpec>): FieldSpec => ({
  id,
  labelKey: `record-field-${id}`,
  control: "text",
  pointer: id,
  read: () => null,
  ...over,
});

// A kind that exists only here: a text, a party and a text with a hint, as a commitment has them.
const text = field("text", { labelKey: "record-field-commitment-text", required: true });
const promisor = field("promisor", {
  labelKey: "record-field-promisor",
  control: "party",
  promptKey: "commitment-field-promisor-placeholder",
  required: true,
});
const condition = field("condition", {
  labelKey: "record-field-condition",
  helpKey: "commitment-field-condition-help",
});

function descriptor(submit: CreateSpec["submit"]): RecordDescriptor {
  return {
    kind: "commitment",
    title: text,
    properties: [promisor, condition],
    evidenceVersionKey: "commitment-evidence-version",
    create: {
      titleKey: "record-create-commitment",
      fields: ["text", "promisor", "condition"],
      defaults: () => ({ text: "" }),
      submit,
    },
  };
}

async function show(submit: (values: Record<string, FieldValue>) => CommandResult<Saved>) {
  const submitted: Record<string, FieldValue>[] = [];
  const onCreated = vi.fn();
  renderWork(
    (api) => (
      <RecordCreate
        descriptor={descriptor(async (_commands, values) => {
          submitted.push(values);
          return submit(values);
        })}
        api={api}
        commands={{} as WorkCommands}
        sources={{ directory: undefined, milestones: undefined }}
        owner={undefined}
        onCreated={onCreated}
        recover={async () => undefined}
        announce={() => {}}
        onCancel={() => {}}
      />
    ),
    {
      lists: {
        "/persons": [],
        "/institutions": [
          { id: "i1", local_id: "INS-001", name: "Testwil Generatoren AG", version: 1 },
        ],
      },
    },
  );
  await screen.findByRole("textbox", { name: /^Zusage/ }, SEARCH);
  return { submitted, onCreated };
}

describe("RecordCreate with a control of its own", () => {
  it("shows a stacked party control with the required mark, and the hint of a field", async () => {
    await show(() => ({ ok: true, record: { local_id: "COM-001" } }));
    expect(screen.getByRole("textbox", { name: "Zusage (Pflichtfeld)" })).toBeVisible();
    expect(screen.getByRole("combobox", { name: "Zugesagt von (Pflichtfeld)" })).toBeVisible();
    expect(screen.getByRole("textbox", { name: "Bedingung" })).toBeVisible();
    expect(
      screen.getByText(
        "Mit einer Bedingung ist die Zusage bedingt. Ohne Bedingung ist sie verbindlich.",
      ),
    ).toBeVisible();
  });

  it("asks for the party, and sends the chosen one as its value", async () => {
    const { submitted, onCreated } = await show(() => ({
      ok: true,
      record: { local_id: "COM-009" },
    }));
    await user.type(screen.getByRole("textbox", { name: /^Zusage/ }), "Lieferung");
    await user.click(screen.getByRole("button", { name: "Erstellen" }));

    const box = screen.getByRole("combobox", { name: /^Zugesagt von/ });
    expect(await screen.findByText("Wählen Sie eine Person oder eine Institution.")).toBeVisible();
    expect(box).toBeInvalid();
    expect(box).toHaveFocus();
    expect(submitted).toEqual([]);

    await user.type(box, "Testwil");
    await user.click(
      await screen.findByRole("option", { name: "Testwil Generatoren AG (INS-001)" }, SEARCH),
    );
    await user.click(screen.getByRole("button", { name: "Erstellen" }));
    await waitFor(() => expect(onCreated).toHaveBeenCalledWith("COM-009"));
    expect(submitted).toEqual([{ text: "Lieferung", promisor: "institution:i1" }]);
  });

  it("puts the error of the server at the party control", async () => {
    await show(() => ({
      ok: false,
      failure: {
        reason: "validation",
        message: "x",
        problem: {
          type: "",
          code: "validation-failed",
          title: "",
          status: 422,
          instance: "",
          request_id: "r",
          errors: [{ pointer: "/promisor", code: "unknown-record" }],
        },
      },
    }));
    await user.type(screen.getByRole("textbox", { name: /^Zusage/ }), "Lieferung");
    await user.type(screen.getByRole("combobox", { name: /^Zugesagt von/ }), "Testwil");
    await user.click(
      await screen.findByRole("option", { name: "Testwil Generatoren AG (INS-001)" }, SEARCH),
    );
    await user.click(screen.getByRole("button", { name: "Erstellen" }));
    expect(await screen.findByText("Diese Person oder Institution gibt es nicht.")).toBeVisible();
    await act(async () => {});
  });
});
