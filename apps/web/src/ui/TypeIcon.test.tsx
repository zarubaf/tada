import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { RecordKind } from "../api/client";
import { RecordId } from "./RecordId";
import { TypeIcon } from "./TypeIcon";

const KINDS: RecordKind[] = ["action", "commitment", "milestone", "decision", "risk"];

describe("TypeIcon", () => {
  it.each([
    ["action", "Aufgabe"],
    ["commitment", "Zusage"],
    ["milestone", "Meilenstein"],
    ["decision", "Entscheid"],
    ["risk", "Risiko"],
  ] as const)("names a %s %s", (kind, name) => {
    render(<TypeIcon kind={kind} />);
    expect(screen.getByRole("img", { name })).toBeInTheDocument();
  });

  it("draws a different icon for each kind", () => {
    const paths = KINDS.map((kind) => {
      const { container, unmount } = render(<TypeIcon kind={kind} />);
      const html = container.querySelector("svg")?.innerHTML;
      unmount();
      return html;
    });
    expect(new Set(paths).size).toBe(KINDS.length);
  });

  it("is decoration next to its name", () => {
    const { container } = render(<TypeIcon kind="risk" decorative />);
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(container.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  });
});

describe("RecordId", () => {
  it("shows the readable ID as text", () => {
    render(<RecordId>ACT-042</RecordId>);
    expect(screen.getByText("ACT-042")).toBeInTheDocument();
  });
});
