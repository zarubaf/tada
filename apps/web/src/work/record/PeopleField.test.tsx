import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it } from "vitest";
import { createApi } from "../../api/client";
import { json } from "../../test/fakeWorkServer";
import { choicesOf } from "./choices";
import type { FieldSpec, FieldValue } from "./descriptor";
import { peopleIds, peopleValue } from "./descriptor";
import { PeopleField } from "./PeopleField";

const user = userEvent.setup({ delay: null });

const persons = [{ id: "p1", local_id: "PER-001", name: "Cäcilia Probst" }];
const institutions = [{ id: "i1", local_id: "INS-001", name: "Testwil Generatoren AG" }];

/** A server that finds the same persons and institutions for every text. */
function fakeApi() {
  const fetch = async (request: Request) => {
    const path = new URL(request.url).pathname;
    return json(200, { items: path.endsWith("/persons") ? persons : institutions });
  };
  return createApi(fetch as unknown as typeof globalThis.fetch);
}

describe("the value of a people field", () => {
  it("is the IDs of the people, joined, and nothing for nobody", () => {
    expect(peopleValue(["person:p1", "institution:i1"])).toBe("person:p1,institution:i1");
    expect(peopleValue([])).toBeNull();
    expect(peopleIds("person:p1,institution:i1")).toEqual(["person:p1", "institution:i1"]);
    expect(peopleIds(null)).toEqual([]);
    expect(peopleIds("")).toEqual([]);
  });
});

describe("the choices of a people field", () => {
  const spec = {
    control: "people",
    people: () => [
      { id: "person:p1", label: "Cäcilia Probst" },
      { id: "institution:i1", label: "Testwil Generatoren AG" },
    ],
  } as unknown as FieldSpec;

  it("are the people of the record, and the field is not offered for a change", () => {
    const { options, offered } = choicesOf(
      spec,
      { directory: undefined, milestones: undefined },
      undefined,
      null,
    );
    expect(options.map((option) => option.label)).toEqual([]);
    expect(offered).toBe(true);
    const item = { kind: "decision" } as never;
    expect(
      choicesOf(spec, { directory: undefined, milestones: undefined }, item, null).options.map(
        (option) => option.id,
      ),
    ).toEqual(["person:p1", "institution:i1"]);
  });
});

function Field({ initial = null }: { initial?: FieldValue }) {
  const [value, setValue] = useState(initial);
  return (
    <>
      <PeopleField label="Beschlossen von" api={fakeApi()} value={value} onChange={setValue} />
      <output data-testid="value">{value ?? "-"}</output>
    </>
  );
}

describe("PeopleField", () => {
  it("searches persons and institutions, and chooses several", async () => {
    render(<Field />);
    const list = await screen.findByRole(
      "listbox",
      { name: "Beschlossen von: Treffer" },
      { timeout: 3000 },
    );
    await within(list).findByRole(
      "option",
      { name: "Cäcilia Probst (PER-001)" },
      { timeout: 3000 },
    );
    await user.click(within(list).getByRole("option", { name: "Cäcilia Probst (PER-001)" }));
    await user.click(
      within(list).getByRole("option", { name: "Testwil Generatoren AG (INS-001)" }),
    );

    expect(screen.getByTestId("value")).toHaveTextContent("person:p1,institution:i1");
    const chosen = screen.getByRole("list", { name: "Beschlossen von: gewählt" });
    expect(within(chosen).getByText("Cäcilia Probst (PER-001)")).toBeVisible();
  });

  it("knows the names of the people of its start value only by the seed, else shows the ID", () => {
    render(
      <PeopleField
        label="Beschlossen von"
        api={fakeApi()}
        value="person:p9"
        onChange={() => undefined}
        seed={[{ id: "person:p9", label: "Bernd Beispiel (PER-009)" }]}
      />,
    );
    expect(screen.getByText("Bernd Beispiel (PER-009)")).toBeVisible();
  });

  it("takes the initials of the plain name, not of the readable ID, in the list and in the results", async () => {
    render(
      <PeopleField
        label="Beschlossen von"
        api={fakeApi()}
        value="institution:i1"
        onChange={() => undefined}
        seed={[
          {
            id: "institution:i1",
            label: "Testwil Generatoren AG (INS-001)",
            name: "Testwil Generatoren AG",
          },
        ]}
      />,
    );
    const chosen = screen.getByRole("list", { name: "Beschlossen von: gewählt" });
    expect(chosen).toHaveTextContent(/^TA/);
    const option = await screen.findByRole(
      "option",
      { name: "Testwil Generatoren AG (INS-001)" },
      { timeout: 3000 },
    );
    expect(option).toHaveTextContent(/^TA/);
  });
});
