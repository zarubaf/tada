import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Kbd } from "./Kbd";

describe("Kbd", () => {
  it("is a kbd element with the key", () => {
    render(<Kbd>Ctrl K</Kbd>);
    expect(screen.getByText("Ctrl K").tagName).toBe("KBD");
  });
});
