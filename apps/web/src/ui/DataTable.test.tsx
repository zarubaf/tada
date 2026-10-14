import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { type Column, DataTable } from "./DataTable";

interface Row {
  id: string;
  name: string;
}

const rows: Row[] = [
  { id: "1", name: "Anna" },
  { id: "2", name: "Bernd" },
];

const columns: Column<Row>[] = [
  { id: "id", header: "ID", cell: (row) => row.id, sortable: true },
  { id: "name", header: "Name", cell: (row) => row.name, sortable: true },
  { id: "note", header: "Notiz", cell: () => "-" },
];

function table(props: Partial<Parameters<typeof DataTable<Row>>[0]> = {}) {
  return render(
    <DataTable label="Leute" columns={columns} rows={rows} rowKey={(row) => row.id} {...props} />,
  );
}

describe("DataTable sorting", () => {
  it("has no buttons in the headers without a sort handler", () => {
    table();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
    expect(screen.getAllByRole("columnheader")).toHaveLength(3);
  });

  it("makes each sortable header a button and says the sorted column in aria-sort", () => {
    table({ sort: { columnId: "name", direction: "desc" }, onSort: () => {} });
    const headers = screen.getAllByRole("columnheader");
    expect(within(headers[0] as HTMLElement).getByRole("button", { name: "ID" })).toBeVisible();
    expect(headers[0]).not.toHaveAttribute("aria-sort");
    expect(headers[1]).toHaveAttribute("aria-sort", "descending");
    // A column that cannot sort has no button.
    expect(within(headers[2] as HTMLElement).queryByRole("button")).not.toBeInTheDocument();
  });

  it("says ascending for the ascending column", () => {
    table({ sort: { columnId: "id", direction: "asc" }, onSort: () => {} });
    expect(screen.getAllByRole("columnheader")[0]).toHaveAttribute("aria-sort", "ascending");
  });

  it("calls onSort with the column on click and on the keyboard", async () => {
    const onSort = vi.fn();
    const user = userEvent.setup();
    table({ sort: { columnId: "id", direction: "asc" }, onSort });
    await user.click(screen.getByRole("button", { name: "Name" }));
    expect(onSort).toHaveBeenLastCalledWith("name");
    await user.tab({ shift: true });
    expect(screen.getByRole("button", { name: "ID" })).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(onSort).toHaveBeenLastCalledWith("id");
    await user.keyboard(" ");
    expect(onSort).toHaveBeenCalledTimes(3);
  });

  it("keeps the two-line rows by default and can turn them off", () => {
    const { container, rerender } = table();
    expect(container.querySelector("table")?.getAttribute("data-stacked")).toBe("");
    rerender(
      <DataTable
        label="Leute"
        columns={columns}
        rows={rows}
        rowKey={(row) => row.id}
        stacked={false}
      />,
    );
    expect(container.querySelector("table")?.hasAttribute("data-stacked")).toBe(false);
  });
});

describe("DataTable headers", () => {
  const selectColumn: Column<Row> = {
    id: "select",
    header: <input type="checkbox" aria-label="Alle auswählen" />,
    label: "Auswahl",
    cell: () => <input type="checkbox" aria-label="Zeile auswählen" />,
  };

  it("takes a node as a header and names the column by its label", () => {
    table({ columns: [selectColumn, ...columns] });
    const headers = screen.getAllByRole("columnheader");
    expect(headers[0]).toHaveAccessibleName("Auswahl");
    expect(within(headers[0] as HTMLElement).getByRole("checkbox")).toBeInTheDocument();
    // A column that is not sortable has no button, also with the sort on.
    expect(within(headers[0] as HTMLElement).queryByRole("button")).not.toBeInTheDocument();
  });

  it("shows the label, not the node, as the name of a cell in a two-line row", () => {
    const { container } = table({ columns: [selectColumn, ...columns] });
    const label = container.querySelector("td span");
    expect(label).toHaveTextContent("Auswahl");
    expect(label?.querySelector("input")).toBeNull();
  });
});

describe("DataTable scrolling", () => {
  let observe: () => void;
  let widths = { scroll: 800, client: 400 };

  function mockObserver() {
    vi.stubGlobal(
      "ResizeObserver",
      class {
        constructor(callback: () => void) {
          observe = callback;
        }
        observe() {
          observe();
        }
        disconnect() {}
      },
    );
    vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockImplementation(() => widths.scroll);
    vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockImplementation(() => widths.client);
  }

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    widths = { scroll: 800, client: 400 };
  });

  it("makes a table that scrolls a focusable region and gives the name to the region only", () => {
    mockObserver();
    table({ stacked: false });
    const region = screen.getByRole("region", { name: "Leute" });
    expect(region).toHaveAttribute("tabindex", "0");
    expect(within(region).getByRole("table")).toBeInTheDocument();
    expect(screen.queryByRole("table", { name: "Leute" })).not.toBeInTheDocument();
  });

  it("is no tab stop and has no region where the columns fit", () => {
    widths = { scroll: 400, client: 400 };
    mockObserver();
    table({ stacked: false });
    expect(screen.queryByRole("region")).not.toBeInTheDocument();
    expect(screen.getByRole("table", { name: "Leute" })).toBeInTheDocument();
    expect(document.querySelector("[tabindex='0']")).toBeNull();
  });

  it("watches the table inside the scroller too, because the table can grow alone", () => {
    const watched: Element[] = [];
    vi.stubGlobal(
      "ResizeObserver",
      class {
        observe(target: Element) {
          watched.push(target);
        }
        disconnect() {}
      },
    );
    table({ stacked: false });
    expect(watched.some((target) => target.tagName === "TABLE")).toBe(true);
  });

  it("follows the width: the region goes away when the columns fit again", () => {
    mockObserver();
    table({ stacked: false });
    expect(screen.getByRole("region", { name: "Leute" })).toBeInTheDocument();
    widths = { scroll: 400, client: 400 };
    act(() => observe());
    expect(screen.queryByRole("region")).not.toBeInTheDocument();
    expect(screen.getByRole("table", { name: "Leute" })).toBeInTheDocument();
  });

  it("has no region around a stacked table", () => {
    mockObserver();
    table();
    expect(screen.queryByRole("region")).not.toBeInTheDocument();
  });
});
