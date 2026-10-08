import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { LazyMarkdown } from "./LazyMarkdown";
import { Markdown } from "./Markdown";

describe("LazyMarkdown", () => {
  it("shows the text and reports that it is shown", async () => {
    const onShown = vi.fn();
    render(<LazyMarkdown onShown={onShown}>{"# Titel"}</LazyMarkdown>);

    expect(await screen.findByRole("heading", { name: "Titel" })).toBeVisible();
    expect(onShown).toHaveBeenCalled();
  });

  it("shows a failure of the chunk with a retry instead of blanking the page", async () => {
    const onShown = vi.fn();
    const load = vi
      .fn()
      .mockRejectedValueOnce(new Error("chunk gone"))
      .mockResolvedValue({ Markdown });
    vi.spyOn(console, "error").mockImplementation(() => {});
    render(
      <LazyMarkdown load={load} onShown={onShown}>
        {"# Titel"}
      </LazyMarkdown>,
    );

    expect(await screen.findByText(/Keine Verbindung zum Server/)).toBeVisible();
    expect(onShown).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Erneut versuchen" }));

    expect(await screen.findByRole("heading", { name: "Titel" })).toBeVisible();
    expect(load).toHaveBeenCalledTimes(2);
    expect(onShown).toHaveBeenCalled();
  });

  it("moves focus to the message when the retry fails again", async () => {
    const load = vi.fn().mockRejectedValue(new Error("chunk gone"));
    vi.spyOn(console, "error").mockImplementation(() => {});
    render(<LazyMarkdown load={load}>{"# Titel"}</LazyMarkdown>);

    await userEvent.click(await screen.findByRole("button", { name: "Erneut versuchen" }));
    await waitFor(() => expect(load).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(screen.getByRole("alert")).toHaveFocus());
  });
});
