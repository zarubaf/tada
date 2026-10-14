import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { type StatusGlyph, StatusIcon } from "./StatusIcon";
import { StatusLabel } from "./StatusLabel";

describe("StatusIcon", () => {
  it("has an accessible name when it has a label", () => {
    render(<StatusIcon glyph="blocked" tone="danger" label="blockiert" />);
    expect(screen.getByRole("img", { name: "blockiert" })).toBeInTheDocument();
  });

  it("is decoration without a label", () => {
    const { container } = render(<StatusIcon glyph="done" tone="success" />);
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(container.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  });

  it("marks the tone for the style and takes the size", () => {
    const { container } = render(<StatusIcon glyph="open" tone="neutral" size={12} />);
    const svg = container.querySelector("svg");
    expect(svg).toHaveAttribute("data-tone", "neutral");
    expect(svg).toHaveAttribute("width", "12");
  });
});

describe("the glyphs", () => {
  // The five icons of ADR 0082.
  it.each([
    ["open", "tabler-icon-circle-dashed"],
    ["progress", "tabler-icon-circle-half-2"],
    ["blocked", "tabler-icon-circle-minus"],
    ["done", "tabler-icon-circle-check-filled"],
    ["canceled", "tabler-icon-circle-x-filled"],
  ] as const)("draws %s as %s", (glyph, iconClass) => {
    const { container } = render(<StatusIcon glyph={glyph} tone="neutral" />);
    expect(container.querySelector("svg")).toHaveClass(iconClass);
  });

  it("draws each glyph with its own icon", () => {
    const glyphs: StatusGlyph[] = [
      "open",
      "progress",
      "blocked",
      "done",
      "canceled",
      "conditional",
      "firm",
      "superseded",
      "mitigated",
      "occurred",
    ];
    const classes = glyphs.map((glyph) => {
      const { container, unmount } = render(<StatusIcon glyph={glyph} tone="neutral" />);
      const icon = container.querySelector("svg")?.getAttribute("class");
      unmount();
      return icon;
    });
    expect(new Set(classes).size).toBe(glyphs.length);
  });

  it("draws a risk that occurred as an octagon, not as the triangle of a conflict", () => {
    const { container } = render(<StatusIcon glyph="occurred" tone="danger" />);
    expect(container.querySelector("svg")).toHaveClass("tabler-icon-alert-octagon");
  });
});

describe("StatusLabel", () => {
  it("draws every icon with the stroke of ADR 0021", () => {
    const { container } = render(
      <StatusLabel icon="progress" tone="progress">
        in Arbeit
      </StatusLabel>,
    );
    expect(container.querySelector("svg")).toHaveAttribute("stroke-width", "1.5");
  });

  it("takes the glyph of a status", () => {
    render(
      <StatusLabel icon="progress" tone="progress">
        in Arbeit
      </StatusLabel>,
    );
    expect(screen.getByText("in Arbeit")).toBeInTheDocument();
  });
});
