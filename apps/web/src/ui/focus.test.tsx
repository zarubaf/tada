import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useRef, useState } from "react";
import { describe, expect, it } from "vitest";
import { ConfirmDialog } from "./ConfirmDialog";
import { firstInvalidField, useFocusAfterCommit } from "./focus";

/** Two frames: more than the one frame in which React Aria restores focus. */
const afterRestore = () =>
  new Promise<void>((resolve) =>
    requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
  );

function RemovableRow() {
  const [removed, setRemoved] = useState(false);
  const heading = useRef<HTMLHeadingElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  return (
    <div>
      <h2 ref={heading} tabIndex={-1}>
        List
      </h2>
      {!removed && (
        <button
          type="button"
          onClick={() => {
            setRemoved(true);
            focusAfterCommit(() => heading.current);
          }}
        >
          Remove
        </button>
      )}
    </div>
  );
}

describe("useFocusAfterCommit", () => {
  it("moves focus to the target when the focused element has left", async () => {
    render(<RemovableRow />);
    await userEvent.click(screen.getByRole("button", { name: "Remove" }));

    expect(screen.queryByRole("button", { name: "Remove" })).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "List" })).toHaveFocus();
  });

  it("moves focus again when the same target is requested a second time", async () => {
    function Again() {
      const focusAfterCommit = useFocusAfterCommit();
      const target = useRef<HTMLButtonElement>(null);
      return (
        <>
          <button type="button" ref={target}>
            Target
          </button>
          <button type="button" onClick={() => focusAfterCommit(() => target.current)}>
            Go
          </button>
        </>
      );
    }
    render(<Again />);
    const go = screen.getByRole("button", { name: "Go" });

    await userEvent.click(go);
    expect(screen.getByRole("button", { name: "Target" })).toHaveFocus();
    go.focus();
    await userEvent.click(go);
    expect(screen.getByRole("button", { name: "Target" })).toHaveFocus();
  });

  it("does nothing when the target does not exist", async () => {
    function Missing() {
      const focusAfterCommit = useFocusAfterCommit();
      return (
        <button type="button" onClick={() => focusAfterCommit(() => null)}>
          Go
        </button>
      );
    }
    render(<Missing />);
    await userEvent.click(screen.getByRole("button", { name: "Go" }));

    expect(screen.getByRole("button", { name: "Go" })).toHaveFocus();
  });

  it("wins over the focus that a closing dialog gives back to its trigger", async () => {
    function WithDialog() {
      const [open, setOpen] = useState(false);
      const heading = useRef<HTMLHeadingElement>(null);
      const focusAfterCommit = useFocusAfterCommit();
      return (
        <div>
          <h2 ref={heading} tabIndex={-1}>
            List
          </h2>
          {/* The trigger stays in the page, so that React Aria can restore focus to it. */}
          <button type="button" onClick={() => setOpen(true)}>
            Open
          </button>
          <ConfirmDialog
            isOpen={open}
            title="Sure?"
            text="Text"
            confirmLabel="Confirm"
            cancelLabel="Cancel"
            onCancel={() => setOpen(false)}
            onConfirm={() => {
              setOpen(false);
              focusAfterCommit(() => heading.current);
            }}
          />
        </div>
      );
    }
    render(<WithDialog />);
    await userEvent.click(screen.getByRole("button", { name: "Open" }));
    await userEvent.click(await screen.findByRole("button", { name: "Confirm" }));

    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    await afterRestore();
    expect(screen.getByRole("heading", { name: "List" })).toHaveFocus();
  });
});

describe("firstInvalidField", () => {
  it("finds the first field with an error", () => {
    render(
      <form aria-label="Form">
        <input aria-label="One" />
        <input aria-label="Two" aria-invalid="true" />
        <input aria-label="Three" aria-invalid="true" />
      </form>,
    );

    expect(firstInvalidField(screen.getByRole("form"))).toBe(screen.getByLabelText("Two"));
  });

  it("returns null without a form or without an error", () => {
    render(<form aria-label="Form" />);

    expect(firstInvalidField(null)).toBeNull();
    expect(firstInvalidField(screen.getByRole("form"))).toBeNull();
  });
});
