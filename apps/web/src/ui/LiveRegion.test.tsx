import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { LiveRegion } from "./LiveRegion";

describe("LiveRegion", () => {
  it("is in the page empty and keeps the same element when its text is set", () => {
    const { rerender } = render(<LiveRegion kind="status" />);
    const region = screen.getByRole("status");
    expect(region).toBeEmptyDOMElement();

    rerender(<LiveRegion kind="status">Saved</LiveRegion>);

    expect(screen.getByRole("status")).toBe(region);
    expect(region).toHaveTextContent("Saved");
  });

  it("is an assertive alert for the kind alert", () => {
    render(<LiveRegion kind="alert">Failed</LiveRegion>);

    expect(screen.getByRole("alert")).toHaveTextContent("Failed");
  });

  it("passes the ref and the class name to the element", () => {
    let element: HTMLParagraphElement | null = null;
    render(
      <LiveRegion
        kind="status"
        className="x"
        ref={(node) => {
          element = node;
        }}
      />,
    );

    expect(element).toBe(screen.getByRole("status"));
    expect(screen.getByRole("status")).toHaveClass("x");
  });
});
