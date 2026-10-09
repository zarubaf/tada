import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ConfirmDialog } from "./ConfirmDialog";

describe("ConfirmDialog", () => {
  it("styles the confirm button of a destructive confirmation as danger", () => {
    render(
      <ConfirmDialog
        isOpen
        title="Entfernen?"
        text="Text"
        confirmLabel="Entfernen"
        cancelLabel="Abbrechen"
        onCancel={() => {}}
        onConfirm={() => {}}
      />,
    );
    expect(screen.getByRole("button", { name: "Entfernen" })).toHaveAttribute(
      "data-variant",
      "danger",
    );
    expect(screen.getByRole("button", { name: "Abbrechen" })).toHaveAttribute(
      "data-variant",
      "secondary",
    );
  });
});
