import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { renderWork } from "../../test/fakeWorkServer";
import { CREATE_CONTROLS } from "./createControls";
import type { FieldSpec } from "./descriptor";
import { PartyControl } from "./PartyControl";

const user = userEvent.setup({ delay: null });
// The search waits 250 ms after the last key; a loaded machine can exceed the default second.
const SEARCH = { timeout: 3000 };

const spec: FieldSpec = {
  id: "promisor",
  labelKey: "record-field-promisor",
  control: "party",
  pointer: "promisor",
  promptKey: "commitment-field-promisor-placeholder",
  required: true,
  read: () => null,
};

const generators = { id: "i1", local_id: "INS-001", name: "Testwil Generatoren AG", version: 1 };
const bakery = { id: "p1", local_id: "PER-001", name: "Beat Bäcker", version: 1 };

async function show(over: { error?: string } = {}) {
  const onChange = vi.fn();
  const { calls } = renderWork(
    (api) => (
      <PartyControl
        api={api}
        spec={spec}
        label="Zugesagt von (Pflichtfeld)"
        value={null}
        error={over.error}
        onChange={onChange}
      />
    ),
    { lists: { "/persons": [bakery], "/institutions": [generators] } },
  );
  await screen.findByRole("combobox", { name: /^Zugesagt von/ }, SEARCH);
  return { calls, onChange };
}

describe("PartyControl", () => {
  it("is a stacked control of the create form", () => {
    expect(CREATE_CONTROLS.party?.stacked).toBe(true);
    expect(CREATE_CONTROLS.party?.Control).toBe(PartyControl);
  });

  it("searches persons and institutions with the typed text and reports the chosen party", async () => {
    const { calls, onChange } = await show();
    const box = screen.getByRole("combobox", { name: "Zugesagt von (Pflichtfeld)" });
    expect(box).toHaveAttribute("placeholder", "Person oder Institution wählen");
    await user.type(box, "Bäck");

    // The search without text lists the person already; click after the typed search was sent.
    await waitFor(
      () =>
        expect(
          calls.some((c) => c.call === "GET /api/v1/persons" && c.query === "?limit=6&q=B%C3%A4ck"),
        ).toBe(true),
      SEARCH,
    );
    await user.click(await screen.findByRole("option", { name: "Beat Bäcker (PER-001)" }, SEARCH));
    expect(onChange).toHaveBeenLastCalledWith("person:p1");
    expect(calls.some((c) => c.call === "GET /api/v1/institutions")).toBe(true);
  });

  it("offers an institution as well, with the kind in the value", async () => {
    const { calls, onChange } = await show();
    await user.type(screen.getByRole("combobox", { name: /^Zugesagt von/ }), "Testwil");
    await waitFor(
      () =>
        expect(
          calls.some(
            (c) => c.call === "GET /api/v1/institutions" && c.query?.includes("q=Testwil"),
          ),
        ).toBe(true),
      SEARCH,
    );
    await user.click(
      await screen.findByRole("option", { name: "Testwil Generatoren AG (INS-001)" }, SEARCH),
    );
    expect(onChange).toHaveBeenLastCalledWith("institution:i1");
  });

  it("marks the field invalid and shows the message", async () => {
    await show({ error: "Wählen Sie eine Person oder eine Institution." });
    expect(screen.getByRole("combobox", { name: /^Zugesagt von/ })).toBeInvalid();
    expect(screen.getByText("Wählen Sie eine Person oder eine Institution.")).toBeVisible();
  });

  it("reports no party when the member clears the text", async () => {
    const { onChange } = await show();
    const box = screen.getByRole("combobox", { name: /^Zugesagt von/ });
    await user.type(box, "Bäck");
    await user.click(await screen.findByRole("option", { name: "Beat Bäcker (PER-001)" }, SEARCH));
    await user.clear(box);
    expect(onChange).toHaveBeenLastCalledWith(null);
  });
});
