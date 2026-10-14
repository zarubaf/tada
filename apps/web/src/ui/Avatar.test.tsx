import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Avatar, avatarColor, initialsOf } from "./Avatar";

describe("initialsOf", () => {
  it.each([
    ["Anna Muster", "AM"],
    ["anna muster", "AM"],
    ["Anna Maria von Muster", "AM"],
    ["Anna", "AN"],
    ["A", "A"],
    ["  Marco   Rossi ", "MR"],
    ["Élodie Zürcher", "ÉZ"],
    ["", "?"],
  ])("makes %j into %j", (name, initials) => {
    expect(initialsOf(name)).toBe(initials);
  });

  it("does not upper-case a letter into two", () => {
    expect(initialsOf("ßenja")).toBe("SE");
    expect(initialsOf("Max Groß")).toBe("MG");
  });

  it("skips what is not a letter", () => {
    expect(initialsOf("Anna (Präsidentin)")).toBe("AP");
    expect(initialsOf("(Anna) Muster")).toBe("AM");
    expect(initialsOf("123")).toBe("?");
    expect(initialsOf("--")).toBe("?");
  });

  it("keeps a letter with its accent in one piece", () => {
    const decomposed = "E\u0301lodie Zu\u0308rcher";
    expect(initialsOf(decomposed)).toBe("\u00c9Z");
    expect(initialsOf("Zoë")).toBe("ZO");
  });

  it("does not break an emoji or a flag", () => {
    expect(initialsOf("🇨🇭 Anna Muster")).toBe("AM");
    expect(initialsOf("👩‍🚀 Anna")).toBe("AN");
    expect(initialsOf("🇨🇭")).toBe("?");
  });
});

describe("avatarColor", () => {
  it("is the same for the same key", () => {
    expect(avatarColor("0198-user-1")).toBe(avatarColor("0198-user-1"));
  });

  it("is one of the six tokens", () => {
    for (const key of ["a", "b", "Anna Muster", "0198-user-1", ""]) {
      expect(avatarColor(key)).toBeGreaterThanOrEqual(1);
      expect(avatarColor(key)).toBeLessThanOrEqual(6);
    }
  });

  it("uses more than one color for different keys", () => {
    const colors = new Set(
      Array.from({ length: 30 }, (_, index) => avatarColor(`person-${index}`)),
    );
    expect(colors.size).toBeGreaterThan(3);
  });
});

describe("Avatar", () => {
  it("is an image named after the person", () => {
    render(<Avatar name="Anna Muster" />);
    expect(screen.getByRole("img", { name: "Anna Muster" })).toHaveTextContent("AM");
  });

  it("is decoration next to the name", () => {
    render(<Avatar name="Anna Muster" decorative />);
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(screen.getByText("AM")).toHaveAttribute("aria-hidden", "true");
  });

  it("is decoration when the name is empty, not an image without a name", () => {
    const { container } = render(<Avatar name="  " />);
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(container.firstElementChild).toHaveAttribute("aria-hidden", "true");
  });

  it("takes its color from the id when there is one", () => {
    const { container } = render(<Avatar name="Anna Muster" id="u-7" />);
    expect(container.firstElementChild).toHaveAttribute("data-color", String(avatarColor("u-7")));
  });

  it("takes the size", () => {
    const { container } = render(<Avatar name="Anna Muster" size={24} />);
    expect(container.firstElementChild).toHaveAttribute("data-size", "24");
  });
});
