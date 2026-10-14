import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { THEME_KEY } from "../theme/theme";
import { ThemeToggle } from "./ThemeToggle";

afterEach(() => {
  localStorage.clear();
  document.documentElement.removeAttribute("data-theme");
});

describe("ThemeToggle", () => {
  it("shows the system choice first, with a visible label", () => {
    render(<ThemeToggle />);
    const button = screen.getByRole("button", { name: "Farbschema: System" });
    expect(button).toHaveTextContent("Farbschema: System");
  });

  it("cycles system, light, dark and stores each choice", async () => {
    const user = userEvent.setup();
    render(<ThemeToggle />);
    const root = document.documentElement;

    await user.click(screen.getByRole("button", { name: "Farbschema: System" }));
    expect(screen.getByRole("button", { name: "Farbschema: Hell" })).toBeInTheDocument();
    expect(root).toHaveAttribute("data-theme", "light");
    expect(localStorage.getItem(THEME_KEY)).toBe("light");

    await user.click(screen.getByRole("button", { name: "Farbschema: Hell" }));
    expect(screen.getByRole("button", { name: "Farbschema: Dunkel" })).toBeInTheDocument();
    expect(root).toHaveAttribute("data-theme", "dark");
    expect(localStorage.getItem(THEME_KEY)).toBe("dark");

    await user.click(screen.getByRole("button", { name: "Farbschema: Dunkel" }));
    expect(screen.getByRole("button", { name: "Farbschema: System" })).toBeInTheDocument();
    expect(root).not.toHaveAttribute("data-theme");
    expect(localStorage.getItem(THEME_KEY)).toBeNull();
  });

  it("announces the new choice in a live region", async () => {
    const user = userEvent.setup();
    render(<ThemeToggle />);
    expect(screen.getByRole("status")).toBeEmptyDOMElement();

    await user.click(screen.getByRole("button", { name: "Farbschema: System" }));
    expect(screen.getByRole("status")).toHaveTextContent("Farbschema geändert: Hell");

    await user.click(screen.getByRole("button", { name: "Farbschema: Hell" }));
    expect(screen.getByRole("status")).toHaveTextContent("Farbschema geändert: Dunkel");
  });

  it("starts with the stored choice", () => {
    localStorage.setItem(THEME_KEY, "dark");
    render(<ThemeToggle />);
    expect(screen.getByRole("button", { name: "Farbschema: Dunkel" })).toBeInTheDocument();
  });
});
