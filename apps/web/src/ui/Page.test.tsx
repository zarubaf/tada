import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { PageTitle } from "./Page";

describe("PageTitle", () => {
  it("marks a compact title that may wrap", () => {
    render(
      <PageTitle compact wrap>
        Ein sehr langer Name
      </PageTitle>,
    );

    const heading = screen.getByRole("heading", { level: 1 });
    expect(heading).toHaveAttribute("data-compact");
    expect(heading).toHaveAttribute("data-wrap");
  });

  it("keeps a compact title on one line by default", () => {
    render(<PageTitle compact>Arbeit</PageTitle>);

    expect(screen.getByRole("heading", { level: 1 })).not.toHaveAttribute("data-wrap");
  });

  it("ignores wrap on a title that is not compact", () => {
    render(<PageTitle wrap>Arbeit</PageTitle>);

    expect(screen.getByRole("heading", { level: 1 })).not.toHaveAttribute("data-wrap");
  });
});
