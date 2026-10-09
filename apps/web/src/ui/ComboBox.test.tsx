import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it } from "vitest";
import { ComboBox } from "./ComboBox";

const user = userEvent.setup({ delay: null });

const ALL = [
  { id: "a", label: "Anna Muster" },
  { id: "b", label: "Bernd Beispiel" },
];

/** The caller filters the options, as a search on the server does. */
function Harness({ error }: { error?: string }) {
  const [text, setText] = useState("");
  const [key, setKey] = useState<string>();
  return (
    <>
      <ComboBox
        label="Person"
        options={ALL.filter((o) => o.label.toLowerCase().includes(text.toLowerCase()))}
        inputValue={text}
        onInputChange={setText}
        selectedKey={key}
        onSelectionChange={setKey}
        error={error}
      />
      <p data-testid="key">{key ?? "none"}</p>
    </>
  );
}

describe("ComboBox", () => {
  it("shows the options that match the text and picks one", async () => {
    render(<Harness />);

    await user.type(screen.getByRole("combobox", { name: "Person" }), "bern");

    expect(screen.queryByRole("option", { name: "Anna Muster" })).not.toBeInTheDocument();
    await user.click(await screen.findByRole("option", { name: "Bernd Beispiel" }));
    expect(screen.getByTestId("key")).toHaveTextContent("b");
    expect(screen.getByRole("combobox", { name: "Person" })).toHaveValue("Bernd Beispiel");
  });

  it("says so when nothing matches", async () => {
    render(<Harness />);

    await user.type(screen.getByRole("combobox", { name: "Person" }), "zzz");

    expect(await screen.findByText("Keine Treffer")).toBeInTheDocument();
  });

  it("shows the error and marks the input as invalid", () => {
    render(<Harness error="Wählen Sie eine Person." />);

    expect(screen.getByText("Wählen Sie eine Person.")).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Person" })).toBeInvalid();
  });
});
