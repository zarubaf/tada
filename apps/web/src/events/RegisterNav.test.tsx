import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { Router } from "../router/Router";
import { EVENT as event } from "../test/fakeWorkServer";
import { EventContext } from "./eventContext";
import { RegisterNav } from "./RegisterNav";

const EVENT = event.id;

function renderAt(path: string) {
  window.history.replaceState(null, "", path);
  render(
    <Router>
      <EventContext
        value={{ event, profile: { kind: "loading" }, reloadProfile: async () => true }}
      >
        <RegisterNav />
      </EventContext>
    </Router>,
  );
}

afterEach(() => window.history.replaceState(null, "", "/"));

describe("RegisterNav", () => {
  it("links each register of the event and marks the current one", () => {
    renderAt(`/events/${EVENT}/milestones`);

    const nav = screen.getByRole("navigation", { name: "Register" });
    const links = within(nav).getAllByRole("link");
    expect(links.map((link) => link.textContent)).toEqual([
      "Zusagen",
      "Meilensteine",
      "Entscheide",
    ]);
    expect(within(nav).getByRole("link", { name: "Meilensteine" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(within(nav).getByRole("link", { name: "Zusagen" })).toHaveAttribute(
      "href",
      `/events/${EVENT}/commitments`,
    );
    expect(within(nav).getByRole("link", { name: "Zusagen" })).not.toHaveAttribute("aria-current");
  });

  it("changes the register without a page load", async () => {
    renderAt(`/events/${EVENT}/commitments`);

    await userEvent.click(screen.getByRole("link", { name: "Meilensteine" }));
    expect(window.location.pathname).toBe(`/events/${EVENT}/milestones`);
    expect(screen.getByRole("link", { name: "Meilensteine" })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });
});
