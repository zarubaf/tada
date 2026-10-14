import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { SidePanel } from "./SidePanel";

const user = userEvent.setup({ delay: null });

function Harness({
  onPrevious,
  onNext,
  removeOpener,
}: {
  onPrevious?: () => void;
  onNext?: () => void;
  removeOpener?: boolean;
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
        </SidePanel>
      )}
      <button type="button">Danach</button>
    </>
  );
}

describe("SidePanel", () => {
  it("is a complementary region named by its title", async () => {
    render(<Harness />);

    await user.click(screen.getByRole("button", { name: "Öffnen" }));

    expect(
      screen.getByRole("complementary", { name: "Sponsorenliste nachführen" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Sponsorenliste nachführen" })).toBeInTheDocument();
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
});
