import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it } from "vitest";
import type { ValueType } from "../api/client";
import { firstInvalidField } from "../ui/focus";
import { ValueInput } from "./ValueInput";
import { type Draft, type DraftField, emptyDraft } from "./valueDraft";

/** Holds the draft, as the form of the Review Inbox does. */
function Harness({
  type,
  errors = {},
  onDraft,
}: {
  type: ValueType;
  errors?: Partial<Record<DraftField, string>>;
  onDraft?: (draft: Draft) => void;
}) {
  const [draft, setDraft] = useState(emptyDraft());
  return (
    <ValueInput
      type={type}
      draft={draft}
      errors={errors}
      onChange={(next) => {
        setDraft(next);
        onDraft?.(next);
      }}
    />
  );
}

describe("ValueInput", () => {
  it("shows a text field for a text and keeps what the member types", async () => {
    let last = emptyDraft();
    render(<Harness type={{ type: "text" }} onDraft={(draft) => (last = draft)} />);
    await userEvent.type(screen.getByRole("textbox", { name: "Text" }), "Hangar 3");
    expect(last.text).toBe("Hangar 3");
  });

  it("shows the currency in the label of an amount", () => {
    render(<Harness type={{ type: "money", currency: "CHF" }} />);
    expect(screen.getByRole("textbox", { name: "Betrag in CHF" })).toBeInTheDocument();
  });

  it("shows a pair of dates and the granularity only if the field does not fix it", () => {
    const { rerender } = render(<Harness type={{ type: "date-window", granularity: null }} />);
    expect(screen.getByLabelText("Beginn")).toHaveAttribute("type", "date");
    expect(screen.getByText("Genauigkeit")).toBeInTheDocument();
    rerender(<Harness type={{ type: "date-window", granularity: "day" }} />);
    expect(screen.queryByText("Genauigkeit")).not.toBeInTheDocument();
  });

  it("shows a checkbox for each value of a multiple choice", () => {
    const type: ValueType = {
      type: "choice",
      multiple: true,
      values: [
        { key: "north", label: { kind: "text", text: "Nordhalle" } },
        { key: "south", label: { kind: "text", text: "Südhalle" } },
      ],
    };
    render(<Harness type={type} />);
    expect(screen.getByRole("checkbox", { name: "Nordhalle" })).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "Südhalle" })).toBeInTheDocument();
  });

  it("marks a field with an error as invalid and shows the message", () => {
    render(<Harness type={{ type: "text" }} errors={{ text: "Geben Sie einen Wert ein." }} />);
    expect(screen.getByRole("textbox", { name: "Text" })).toHaveAttribute("aria-invalid", "true");
    expect(screen.getByText("Geben Sie einen Wert ein.")).toBeInTheDocument();
  });

  it("shows the error of a select and the first invalid field is its button", () => {
    const { container } = render(
      <Harness type={{ type: "boolean" }} errors={{ flag: "Geben Sie einen Wert ein." }} />,
    );
    expect(screen.getByText("Geben Sie einen Wert ein.")).toBeInTheDocument();
    expect(firstInvalidField(container)).toBe(screen.getByRole("button", { name: /Antwort/ }));
  });

  it("offers the approximate mark", async () => {
    let last = emptyDraft();
    render(<Harness type={{ type: "text" }} onDraft={(draft) => (last = draft)} />);
    await userEvent.click(screen.getByRole("checkbox", { name: "Ungefährer Wert" }));
    expect(last.approximate).toBe(true);
  });
});
