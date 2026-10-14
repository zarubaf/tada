import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Chip } from "./Chip";

describe("Chip", () => {
  it("is static text without a press handler", () => {
    render(<Chip>Wirkung hoch</Chip>);
    expect(screen.getByText("Wirkung hoch")).toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("is a button with a press handler", async () => {
    const onPress = vi.fn();
    render(
      <Chip variant="workstream" onPress={onPress}>
        Infrastruktur
      </Chip>,
    );
    await userEvent.click(screen.getByRole("button", { name: /Infrastruktur/ }));
    expect(onPress).toHaveBeenCalledOnce();
  });

  describe("date", () => {
    it("shows the date in a time element", () => {
      render(<Chip variant="date" date="2026-10-15" today="2026-10-14" />);
      const time = screen.getByText("15.10.2026");
      expect(time.tagName).toBe("TIME");
      expect(time).toHaveAttribute("datetime", "2026-10-15");
      expect(screen.queryByText(/überfällig/)).not.toBeInTheDocument();
    });

    it("is not overdue on the day itself", () => {
      render(<Chip variant="date" date="2026-10-14" today="2026-10-14" />);
      expect(screen.queryByText(/überfällig/)).not.toBeInTheDocument();
    });

    it("says überfällig in words when the date is before today", () => {
      const { container } = render(<Chip variant="date" date="2026-10-12" today="2026-10-14" />);
      expect(screen.getByText(/überfällig/)).toBeInTheDocument();
      expect(container.firstElementChild).toHaveAttribute("data-overdue", "true");
    });

    it("takes today from the browser day when none is given", () => {
      render(<Chip variant="date" date="2000-01-01" />);
      expect(screen.getByText(/überfällig/)).toBeInTheDocument();
    });
  });

  it("marks an interactive chip for its hit area", () => {
    const { container, rerender } = render(<Chip>Wirkung hoch</Chip>);
    expect(container.firstElementChild).not.toHaveAttribute("data-interactive");
    rerender(<Chip onPress={() => {}}>Wirkung hoch</Chip>);
    expect(container.firstElementChild).toHaveAttribute("data-interactive", "true");
  });

  it("takes overdue and dateTime only on the date variant (checked by the compiler)", () => {
    const rejected = () => (
      <>
        {/* @ts-expect-error only the date chip is overdue */}
        <Chip variant="workstream" overdue>
          Technik
        </Chip>
        {/* @ts-expect-error only the date chip has a date time */}
        <Chip variant="workstream" dateTime="2026-10-15">
          Technik
        </Chip>
        {/* @ts-expect-error a date chip needs a date */}
        <Chip variant="date" />
      </>
    );
    expect(rejected).toBeTypeOf("function");
  });

  it("names a workstream as a Bereich", () => {
    render(<Chip variant="workstream">Technik</Chip>);
    expect(screen.getByText(/Bereich/)).toBeInTheDocument();
    expect(screen.getByText("Technik")).toBeInTheDocument();
  });

  it.each([
    ["proposal", "Vorschlag"],
    ["assumption", "Annahme"],
    ["unknown", "Unbekannt"],
  ] as const)("shows the label %s of the state of knowledge", (variant, label) => {
    render(<Chip variant={variant} />);
    expect(screen.getByText(label)).toBeInTheDocument();
  });

  it("adds the detail of a proposal after the label", () => {
    render(<Chip variant="proposal">16.10.</Chip>);
    expect(screen.getByText("Vorschlag · 16.10.")).toBeInTheDocument();
  });
});
