import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it } from "vitest";
import { SearchField } from "./SearchField";

function Harness() {
  const [value, setValue] = useState("");
  return <SearchField label="Arbeit durchsuchen" value={value} onChange={setValue} />;
}

describe("SearchField", () => {
  it("is a search box with the given name that holds the typed text", async () => {
    render(<Harness />);
    await userEvent.type(screen.getByRole("searchbox", { name: "Arbeit durchsuchen" }), "Zelt");
    expect(screen.getByRole("searchbox")).toHaveValue("Zelt");
  });

  it("clears with its button", async () => {
    render(<Harness />);
    await userEvent.type(screen.getByRole("searchbox"), "Zelt");
    await userEvent.click(screen.getByRole("button", { name: "Suche leeren" }));
    expect(screen.getByRole("searchbox")).toHaveValue("");
  });
});
