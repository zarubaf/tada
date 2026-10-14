import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { RecordStatusIcon, RecordStatusLabel } from "./WorkStatus";

describe("RecordStatusLabel", () => {
  it("shows the German word of the status with an icon", () => {
    const { container } = render(<RecordStatusLabel kind="risk" status="occurred" />);
    expect(screen.getByText("eingetreten")).toBeInTheDocument();
    expect(container.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  });
});

describe("RecordStatusIcon", () => {
  it("names the status for a screen reader", () => {
    render(<RecordStatusIcon kind="commitment" status="conditional" />);
    expect(screen.getByRole("img", { name: "bedingt" })).toBeInTheDocument();
  });
});
