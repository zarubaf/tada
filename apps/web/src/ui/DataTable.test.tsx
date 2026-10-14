import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
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
