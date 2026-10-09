import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Markdown } from "./Markdown";

describe("Markdown", () => {
  it("renders headings one level lower, because the page has the h1", () => {
    render(<Markdown>{"# Datenschutz\n\n## Zwecke"}</Markdown>);
    expect(screen.getByRole("heading", { level: 2, name: "Datenschutz" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 3, name: "Zwecke" })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { level: 1 })).not.toBeInTheDocument();
  });

  it("renders lists and tables", () => {
    render(
      <Markdown>{"- Eins\n- Zwei\n\n| Wer | Was |\n| --- | --- |\n| Verein | Daten |"}</Markdown>,
    );
    expect(screen.getAllByRole("listitem")).toHaveLength(2);
    expect(screen.getByRole("table")).toBeInTheDocument();
    expect(screen.getByRole("columnheader", { name: "Wer" })).toBeInTheDocument();
  });

  it("never makes elements of raw HTML", () => {
    const { container } = render(
      <Markdown>
        {'Text <script>window.hacked = 1</script><b onclick="x()">fett</b>\n\n<div>Block</div>'}
      </Markdown>,
    );
    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector("b")).toBeNull();
    // Only the wrapper of the component is a div.
    expect(container.querySelectorAll("div")).toHaveLength(1);
    expect(container.innerHTML).not.toContain("onclick");
  });

  it("renders an image as its alternative text and never loads it", () => {
    const { container } = render(
      <Markdown>{"![Lageplan](https://example.org/plan.png)"}</Markdown>,
    );
    expect(container.querySelector("img")).toBeNull();
    expect(screen.getByText("Lageplan")).toBeInTheDocument();
  });

  it("opens an external link safely", () => {
    render(<Markdown>{"[Beispiel](https://example.org/a)"}</Markdown>);
    const link = screen.getByRole("link", { name: "Beispiel" });
    expect(link).toHaveAttribute("href", "https://example.org/a");
    expect(link).toHaveAttribute("rel", "noopener noreferrer");
    expect(link).toHaveAttribute("target", "_blank");
  });

  it("keeps a mailto link", () => {
    render(<Markdown>{"[Post](mailto:vorstand@example.org)"}</Markdown>);
    expect(screen.getByRole("link", { name: "Post" })).toHaveAttribute(
      "href",
      "mailto:vorstand@example.org",
    );
  });

  it.each([
    "javascript:alert(1)",
    "http://example.org",
    "data:text/html,x",
    "/settings/members",
    "#anker",
  ])("shows the link %s as text, without an address", (destination) => {
    render(<Markdown>{`[Wort](${destination})`}</Markdown>);
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
    expect(screen.getByText("Wort")).toBeInTheDocument();
  });

  it("does not follow a tada link as an address", () => {
    render(<Markdown>{"[Quelle](tada:source/0198f5a6-7c1e-7000-8000-000000000001#0-5)"}</Markdown>);
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
    expect(screen.getByText("Quelle")).toBeInTheDocument();
  });

  it("hands a tada link to the renderer of the page, with its destination", () => {
    render(
      <Markdown renderLink={(href, children) => <b data-href={href}>{children}!</b>}>
        {"[Quelle](tada:source/0198f5a6-7c1e-7000-8000-000000000001#0-5) und [](tada:fact/x?v=1)"}
      </Markdown>,
    );
    expect(screen.getByText("Quelle!")).toHaveAttribute(
      "data-href",
      "tada:source/0198f5a6-7c1e-7000-8000-000000000001#0-5",
    );
    expect(screen.getByText("!")).toHaveAttribute("data-href", "tada:fact/x?v=1");
  });

  it("does not hand any other link to the renderer of the page", () => {
    render(
      <Markdown renderLink={() => <i>falsch</i>}>
        {"[Beispiel](https://example.org) [Skript](javascript:alert(1))"}
      </Markdown>,
    );
    expect(screen.queryByText("falsch")).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Beispiel" })).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "Skript" })).not.toBeInTheDocument();
  });

  it("shifts the headings to the level that the page needs, and stops at h6", () => {
    render(<Markdown headingLevel={5}>{"# Eins\n\n## Zwei\n\n### Drei"}</Markdown>);
    expect(screen.getByRole("heading", { level: 5, name: "Eins" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 6, name: "Zwei" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 6, name: "Drei" })).toBeInTheDocument();
  });
});
