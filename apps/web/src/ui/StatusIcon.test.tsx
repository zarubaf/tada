import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { StatusIcon } from "./StatusIcon";
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

describe("StatusLabel", () => {
  it("takes the glyph of a status", () => {
    render(
      <StatusLabel icon="progress" tone="progress">
        in Arbeit
      </StatusLabel>,
    );
    expect(screen.getByText("in Arbeit")).toBeInTheDocument();
  });
});
