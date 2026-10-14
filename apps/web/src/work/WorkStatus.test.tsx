import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import {
  ActionStatusLabel,
  RecordStatusIcon,
  RecordStatusLabel,
  RiskStatusLabel,
} from "./WorkStatus";

describe("RecordStatusLabel", () => {
  it("shows the German word of the status with an icon", () => {
    const { container } = render(<RecordStatusLabel kind="risk" status="occurred" />);
    expect(screen.getByText("eingetreten")).toBeInTheDocument();
    expect(container.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  });

  it("is the same as the label of the kind", () => {
    const { container: a } = render(<RecordStatusLabel kind="action" status="blocked" />);
    const { container: b } = render(<ActionStatusLabel status="blocked" />);
    expect(a.innerHTML).toBe(b.innerHTML);
  });

  it("has a label for a risk", () => {
    render(<RiskStatusLabel status="mitigated" />);
    expect(screen.getByText("entschärft")).toBeInTheDocument();
  });
});

describe("RecordStatusIcon", () => {
  it("names the status for a screen reader", () => {
    render(<RecordStatusIcon kind="commitment" status="conditional" />);
    expect(screen.getByRole("img", { name: "bedingt" })).toBeInTheDocument();
  });
});
