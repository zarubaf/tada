import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it } from "vitest";
import { TabPanel, Tabs } from "./Tabs";

const user = userEvent.setup({ delay: null });

const TABS = [
  { id: "board", label: "Board" },
  { id: "table", label: "Tabelle", count: 12 },
  { id: "plan", label: "Zeitplan" },
];

function Harness({ look, panels }: { look?: "view" | "filter"; panels?: boolean }) {
  const [value, setValue] = useState("board");
  return (
    <>
      <Tabs label="Ansicht" tabs={TABS} value={value} onChange={setValue} look={look}>
        {panels && (
          <>
            <TabPanel id="board">Spalten</TabPanel>
            <TabPanel id="table">Zeilen</TabPanel>
            <TabPanel id="plan">Wochen</TabPanel>
          </>
        )}
      </Tabs>
      <p data-testid="value">{value}</p>
    </>
  );
}

describe("Tabs", () => {
  it("marks the selected tab and shows the counts", () => {
    render(<Harness />);

    expect(screen.getByRole("tablist", { name: "Ansicht" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Board" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tab", { name: "Tabelle 12" })).toHaveAttribute(
      "aria-selected",
      "false",
    );
  });

  it("moves the selection with the arrow keys and reports it", async () => {
    render(<Harness />);

    await user.tab();
    expect(screen.getByRole("tab", { name: "Board" })).toHaveFocus();
    await user.keyboard("{ArrowRight}");

    expect(screen.getByRole("tab", { name: "Tabelle 12" })).toHaveFocus();
    expect(screen.getByTestId("value")).toHaveTextContent("table");
  });

  it("has one tab stop", async () => {
    render(<Harness />);

    await user.tab();
    await user.tab();

    expect(screen.getByRole("tab", { name: "Board" })).not.toHaveFocus();
  });

  it("works as a type filter without panels", async () => {
    render(<Harness look="filter" />);

    await user.click(screen.getByRole("tab", { name: "Zeitplan" }));

    expect(screen.getByTestId("value")).toHaveTextContent("plan");
    expect(screen.queryByRole("tabpanel")).not.toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Zeitplan" })).not.toHaveAttribute("aria-controls");
  });

  it("shows the panel of the selected tab", async () => {
    render(<Harness panels />);

    expect(screen.getByRole("tabpanel")).toHaveTextContent("Spalten");
    await user.click(screen.getByRole("tab", { name: "Tabelle 12" }));
    expect(screen.getByRole("tabpanel")).toHaveTextContent("Zeilen");
  });
});
