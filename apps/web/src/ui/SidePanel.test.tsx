import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SearchSelect } from "./SearchSelect";
import { SidePanel } from "./SidePanel";

const user = userEvent.setup({ delay: null });

function Harness({
  onPrevious,
  onNext,
  removeOpener,
  closeFromOutside,
}: {
  onPrevious?: () => void;
  onNext?: () => void;
  removeOpener?: boolean;
  closeFromOutside?: boolean;
}) {
  const [open, setOpen] = useState(false);
  return (
    <>
      {!(open && removeOpener) && (
        <button type="button" onClick={() => setOpen(true)}>
          Öffnen
        </button>
      )}
      <button type="button" id="list">
        Liste
      </button>
      {open && (
        <SidePanel
          title="Sponsorenliste nachführen"
          onClose={() => setOpen(false)}
          returnFocusTo={() => document.getElementById("list")}
          {...(onPrevious ? { onPrevious } : {})}
          {...(onNext ? { onNext } : {})}
        >
          <button type="button">Im Panel</button>
          <SearchSelect
            label="Zuständig"
            placeholder="Niemand"
            options={[{ id: "a", label: "Anna Muster" }]}
            value={undefined}
            onChange={() => {}}
          />
        </SidePanel>
      )}
      {closeFromOutside && (
        <button type="button" onClick={() => setOpen(false)}>
          Von aussen schliessen
        </button>
      )}
      <button type="button">Danach</button>
    </>
  );
}

/** A viewport that starts narrow or wide and can change. */
function mockViewport(narrow: boolean) {
  const listeners = new Set<() => void>();
  const query = {
    matches: narrow,
    addEventListener: (_type: string, listener: () => void) => listeners.add(listener),
    removeEventListener: (_type: string, listener: () => void) => listeners.delete(listener),
  };
  vi.stubGlobal("matchMedia", () => query);
  return (next: boolean) => {
    query.matches = next;
    act(() => {
      for (const listener of listeners) {
        listener();
      }
    });
  };
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("SidePanel", () => {
  it("is a complementary region named by its title", async () => {
    render(<Harness />);

    await user.click(screen.getByRole("button", { name: "Öffnen" }));

    expect(
      screen.getByRole("complementary", { name: "Sponsorenliste nachführen" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Sponsorenliste nachführen" })).toBeInTheDocument();
  });

  it("shows the heading of the caller in the head and keeps the title as the name", async () => {
    render(
      <SidePanel title="ACT-012 Sponsorenliste" heading={<span>Kopf</span>} onClose={() => {}}>
        Inhalt
      </SidePanel>,
    );

    expect(screen.getByRole("complementary", { name: "ACT-012 Sponsorenliste" })).toBeVisible();
    expect(screen.getByText("Kopf")).toBeVisible();
    expect(screen.queryByRole("heading", { name: "ACT-012 Sponsorenliste" })).toBeNull();
  });

  it("names the buttons of the head and gives them a tooltip", async () => {
    render(
      <SidePanel title="A" onClose={() => {}} onPrevious={() => {}} onNext={() => {}}>
        x
      </SidePanel>,
    );

    await user.tab();
    expect(screen.getByRole("button", { name: "Vorheriger Eintrag" })).toHaveFocus();
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Vorheriger Eintrag");
  });

  it("moves focus into the panel on open", async () => {
    render(<Harness />);

    await user.click(screen.getByRole("button", { name: "Öffnen" }));

    expect(screen.getByRole("complementary")).toHaveFocus();
  });

  it("closes on Escape and returns focus to the opener", async () => {
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Öffnen" }));

    await user.keyboard("{Escape}");

    expect(screen.queryByRole("complementary")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Öffnen" })).toHaveFocus();
  });

  it("closes with the close button", async () => {
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Öffnen" }));

    await user.click(screen.getByRole("button", { name: "Schliessen" }));

    expect(screen.queryByRole("complementary")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Öffnen" })).toHaveFocus();
  });

  it("returns focus to the fallback when the opener is gone", async () => {
    render(<Harness removeOpener />);
    await user.click(screen.getByRole("button", { name: "Öffnen" }));

    await user.keyboard("{Escape}");

    expect(screen.getByRole("button", { name: "Liste" })).toHaveFocus();
  });

  it("does not trap focus", async () => {
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Öffnen" }));

    await user.tab();
    expect(screen.getByRole("button", { name: "Schliessen" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "Im Panel" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: /Zuständig/ })).toHaveFocus();
    await user.tab();

    expect(screen.getByRole("button", { name: "Danach" })).toHaveFocus();
  });

  it("shows previous and next only when the page handles them", async () => {
    const onNext = vi.fn();
    const { unmount } = render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Öffnen" }));
    expect(screen.queryByRole("button", { name: "Nächster Eintrag" })).not.toBeInTheDocument();
    unmount();

    render(<Harness onNext={onNext} onPrevious={() => {}} />);
    await user.click(screen.getByRole("button", { name: "Öffnen" }));
    await user.click(screen.getByRole("button", { name: "Nächster Eintrag" }));

    expect(onNext).toHaveBeenCalledOnce();
    expect(screen.getByRole("button", { name: "Vorheriger Eintrag" })).toBeInTheDocument();
  });

  it("keeps both step buttons when one end is reached, the missing one disabled", async () => {
    const onNext = vi.fn();
    render(<Harness onNext={onNext} />);
    await user.click(screen.getByRole("button", { name: "Öffnen" }));

    const previous = screen.getByRole("button", { name: "Vorheriger Eintrag" });
    expect(previous).toHaveAttribute("aria-disabled", "true");
    previous.focus();
    expect(previous).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(onNext).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Nächster Eintrag" })).not.toHaveAttribute(
      "aria-disabled",
      "true",
    );
  });

  it("keeps focus where it is when the page closes the panel", async () => {
    render(<Harness closeFromOutside />);
    await user.click(screen.getByRole("button", { name: "Öffnen" }));

    await user.click(screen.getByRole("button", { name: "Von aussen schliessen" }));

    expect(screen.queryByRole("complementary")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Von aussen schliessen" })).toHaveFocus();
  });

  it("closes only the popover on Escape inside a popover", async () => {
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Öffnen" }));
    await user.click(screen.getByRole("button", { name: /Zuständig/ }));
    await screen.findByRole("listbox");

    await user.keyboard("{Escape}");

    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(screen.getByRole("complementary")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Zuständig/ })).toHaveFocus();
  });

  it("makes the page behind it inert and stops its scroll below the medium breakpoint", async () => {
    mockViewport(true);
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Öffnen" }));

    expect((screen.getByRole("button", { name: "Liste" }) as HTMLElement).inert).toBe(true);
    expect((screen.getByRole("complementary") as HTMLElement).inert).not.toBe(true);
    expect(document.documentElement.style.overflow).toBe("hidden");

    await user.keyboard("{Escape}");

    expect((screen.getByRole("button", { name: "Liste" }) as HTMLElement).inert).toBe(false);
    expect(document.documentElement.style.overflow).toBe("");
  });

  it("follows the viewport: the page is usable again when the panel is an overlay", async () => {
    const setNarrow = mockViewport(true);
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Öffnen" }));
    expect((screen.getByRole("button", { name: "Liste" }) as HTMLElement).inert).toBe(true);

    setNarrow(false);

    expect((screen.getByRole("button", { name: "Liste" }) as HTMLElement).inert).toBe(false);
    expect(document.documentElement.style.overflow).toBe("");
  });

  it("leaves the page alone on a wide viewport", async () => {
    mockViewport(false);
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Öffnen" }));

    expect((screen.getByRole("button", { name: "Liste" }) as HTMLElement).inert).toBeFalsy();
  });
});

describe("SidePanel below the medium breakpoint, in a shell", () => {
  function Shell({ swap }: { swap?: boolean }) {
    return (
      <div id="root">
        <header id="top">
          <button type="button" id="org">
            Organisation
          </button>
          <p id="live" role="status" />
        </header>
        <nav id="nav">
          <a href="/x">Navigation</a>
        </nav>
        <div id="wrap">
          <button type="button" id="beside">
            Daneben
          </button>
          <div id="toasts" data-toast-region="" />
        </div>
        <main id="main">
          {swap ? <p id="empty">Leer</p> : <ul id="list" />}
          <SidePanel title="Eintrag" onClose={() => {}}>
            <button type="button">Im Panel</button>
            <SearchSelect
              label="Zuständig"
              placeholder="Niemand"
              options={[{ id: "a", label: "Anna Muster" }]}
              value={undefined}
              onChange={() => {}}
            />
          </SidePanel>
        </main>
      </div>
    );
  }
  const inert = (id: string) => (document.getElementById(id) as HTMLElement).inert;

  it("makes every sibling along the ancestors inert, except live regions", () => {
    mockViewport(true);
    render(<Shell />);

    expect(inert("top")).toBeFalsy();
    expect(inert("org")).toBe(true);
    expect(inert("nav")).toBe(true);
    expect(inert("list")).toBe(true);
    expect(inert("live")).toBeFalsy();
    expect(inert("wrap")).toBeFalsy();
    expect(inert("beside")).toBe(true);
    expect(inert("toasts")).toBeFalsy();
    expect(inert("main")).toBeFalsy();
    expect(inert("root")).toBeFalsy();
    expect((screen.getByRole("complementary") as HTMLElement).inert).toBeFalsy();
  });

  it("restores everything on unmount", () => {
    mockViewport(true);
    const { unmount } = render(<Shell />);
    const nav = document.getElementById("nav") as HTMLElement;
    expect(nav.inert).toBe(true);

    unmount();

    expect(nav.inert).toBe(false);
  });

  it("restores the elements when the viewport grows", () => {
    const setNarrow = mockViewport(true);
    render(<Shell />);

    setNarrow(false);

    expect(inert("org")).toBe(false);
    expect(inert("beside")).toBe(false);
    expect(inert("nav")).toBe(false);
    expect(inert("list")).toBe(false);
  });

  it("marks a sibling that appears later inside a header that holds a live region", async () => {
    mockViewport(true);
    render(<Shell />);
    const added = document.createElement("button");
    added.id = "late-in-header";
    document.getElementById("top")?.append(added);
    await waitFor(() => expect(inert("late-in-header")).toBe(true));
    expect(inert("top")).toBeFalsy();
  });

  it("leaves a portal root that appears later in the body alone", async () => {
    mockViewport(true);
    render(<Shell />);
    const portal = document.createElement("div");
    portal.id = "portal";
    document.body.append(portal);
    // Let the observer run; add a sentinel sibling that must still be made inert.
    const late = document.createElement("section");
    late.id = "late-main";
    document.getElementById("main")?.prepend(late);
    await waitFor(() => expect(inert("late-main")).toBe(true));

    expect(inert("portal")).toBeFalsy();
    portal.remove();
  });

  it("keeps the popover of a select inside the panel out of reach of inert", async () => {
    mockViewport(true);
    render(<Shell />);
    await user.click(screen.getByRole("button", { name: /Zuständig/ }));
    const listbox = await screen.findByRole("listbox");

    for (let node: HTMLElement | null = listbox; node; node = node.parentElement) {
      expect(node.inert, node.outerHTML.slice(0, 60)).toBeFalsy();
      if (node === document.body) {
        break;
      }
    }
  });

  it("marks a sibling that appears later, and releases it on close", async () => {
    mockViewport(true);
    const { unmount } = render(<Shell />);
    const added = document.createElement("section");
    added.id = "late";
    document.getElementById("main")?.prepend(added);
    await waitFor(() => expect(inert("late")).toBe(true));

    unmount();

    expect(added.inert).toBe(false);
  });

  it("marks the new root when the list swaps its root element", async () => {
    mockViewport(true);
    const { rerender } = render(<Shell />);

    rerender(<Shell swap />);

    await waitFor(() => expect(inert("empty")).toBe(true));
  });
});
