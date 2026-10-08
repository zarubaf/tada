import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { KnowledgeState } from "./KnowledgeState";

describe("KnowledgeState", () => {
  it.each([
    ["accepted", "Bestätigt"],
    ["proposed", "Vorschlag"],
    ["assumption", "Annahme"],
  ] as const)("shows the value and the label %s", (state, label) => {
    render(<KnowledgeState state={state}>Flugplatz Testwil</KnowledgeState>);
    expect(screen.getByText("Flugplatz Testwil")).toBeInTheDocument();
    expect(screen.getByText(label)).toBeInTheDocument();
  });

  it("shows Unbekannt once for an unknown value", () => {
    render(<KnowledgeState state="unknown" />);
    expect(screen.getAllByText("Unbekannt")).toHaveLength(1);
  });

  it("marks the state for the style", () => {
    const { container } = render(<KnowledgeState state="assumption">20</KnowledgeState>);
    expect(container.firstElementChild).toHaveAttribute("data-state", "assumption");
  });
});
