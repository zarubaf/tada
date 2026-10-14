import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import * as r from "../../test/workRecords";
import { toWorkItem } from "../workItem";
import type { FieldSpec } from "./descriptor";
import { FieldEditor, type FieldEnv } from "./FieldEditor";

const user = userEvent.setup({ delay: null });

const item = toWorkItem(
  "decision",
  r.decision("DEC-001", { superseded_by: "d2" }) as never,
  () => "",
);

const successor: FieldSpec = {
  id: "successor",
  labelKey: "decision-superseded-by",
  control: "record",
  pointer: "superseded_by",
  refers: "decision",
  read: () => "d2",
};

function show(known: boolean) {
  const open = vi.fn();
  const env = {
    sources: {
      directory: undefined,
      milestones: undefined,
      records: {
        readableId: (_kind: string, id: string) => (known && id === "d2" ? "DEC-002" : undefined),
      },
    },
    editable: true,
    writable: true,
    open,
  } as unknown as FieldEnv;
  render(<FieldEditor spec={successor} item={item} env={env} />);
  return { open };
}

describe("a field that refers to another record", () => {
  it("shows the readable ID as a link that opens the record in the panel", async () => {
    const { open } = show(true);
    const link = screen.getByRole("link", { name: "DEC-002" });
    expect(link).toHaveAttribute("href", "#DEC-002");
    await user.click(link);
    expect(open).toHaveBeenCalledWith("DEC-002");
  });

  it("leaves the click with a modifier to the browser", async () => {
    const { open } = show(true);
    await user.keyboard("{Control>}");
    await user.click(screen.getByRole("link", { name: "DEC-002" }));
    expect(open).not.toHaveBeenCalled();
  });

  it("says so when the record is not loaded", () => {
    show(false);
    expect(screen.queryByRole("link")).toBeNull();
    expect(screen.getByText("Unbekannter Eintrag")).toBeVisible();
  });
});
