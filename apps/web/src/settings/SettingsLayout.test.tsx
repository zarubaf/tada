import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { Router } from "../router/Router";
import { SettingsLayout } from "./SettingsLayout";

afterEach(() => window.history.replaceState(null, "", "/"));

describe("SettingsLayout", () => {
  it("links the settings pages and marks the current one", () => {
    window.history.replaceState(null, "", "/settings/telegram");
    render(
      <Router>
        <SettingsLayout>
          <p>Inhalt</p>
        </SettingsLayout>
      </Router>,
    );

    expect(screen.getByRole("navigation", { name: "Einstellungen" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Mitglieder" })).toHaveAttribute(
      "href",
      "/settings/members",
    );
    expect(screen.getByRole("link", { name: "Telegram" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("link", { name: "Mitglieder" })).not.toHaveAttribute("aria-current");
    expect(screen.getByText("Inhalt")).toBeInTheDocument();
  });
});
