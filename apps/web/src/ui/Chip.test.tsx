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
      render(
        <Chip variant="date" dateTime="2026-10-15">
          15.10.
        </Chip>,
      );
      const time = screen.getByText("15.10.");
      expect(time.tagName).toBe("TIME");
      expect(time).toHaveAttribute("datetime", "2026-10-15");
      expect(screen.queryByText(/überfällig/)).not.toBeInTheDocument();
    });

    it("says überfällig in words when it is overdue", () => {
      const { container } = render(
        <Chip variant="date" dateTime="2026-10-12" overdue>
          12.10.
        </Chip>,
      );
      expect(screen.getByText(/überfällig/)).toBeInTheDocument();
      expect(container.firstElementChild).toHaveAttribute("data-overdue", "true");
    });
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
