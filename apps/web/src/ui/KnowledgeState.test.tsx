import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { KNOWLEDGE_ICONS, KnowledgeState } from "./KnowledgeState";

describe("KnowledgeState", () => {
  it.each([
    ["accepted", "Bestätigt"],
    ["proposed", "Vorschlag"],
    ["assumption", "Annahme"],
    ["conflict", "Konflikt"],
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

  it("draws each state with its own icon", () => {
    expect(new Set(Object.values(KNOWLEDGE_ICONS)).size).toBe(Object.keys(KNOWLEDGE_ICONS).length);
  });

  it("draws a proposal as a dashed square, not as the dashed circle of an open status", () => {
    const { container } = render(<KnowledgeState state="proposed">x</KnowledgeState>);
    expect(container.querySelector("svg")).toHaveClass("tabler-icon-square-dashed");
  });
});
