import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { SkipLink } from "./SkipLink";

afterEach(() => {
  document.body.innerHTML = "";
  window.history.replaceState(null, "", "/");
});

describe("SkipLink", () => {
  it("focuses the main element and leaves the address as it is", () => {
    window.history.replaceState(null, "", "/events/e1/work?view=table");
    render(
      <>
        <SkipLink />
        <main id="main">content</main>
      </>,
    );
    const notPrevented = fireEvent.click(screen.getByRole("link"));
    expect(notPrevented).toBe(false);
    expect(document.activeElement).toBe(document.querySelector("main"));
    expect(window.location.pathname + window.location.search + window.location.hash).toBe(
      "/events/e1/work?view=table",
    );
  });
});
