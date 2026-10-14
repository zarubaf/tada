import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useDebouncedText } from "./useDebouncedText";

const commit = vi.fn();

/** The committed text lives in the harness, as it lives in the query string of the page. */
function Harness({ initial = "" }: { initial?: string }) {
  const [committed, setCommitted] = useState(initial);
  const field = useDebouncedText(committed, (text) => {
    commit(text);
    setCommitted(text);
  });
  return (
    <>
      <input
        aria-label="Suche"
        value={field.draft}
        onChange={(event) => field.change(event.target.value)}
        onBlur={field.flush}
        onKeyDown={(event) => event.key === "Enter" && field.flush()}
      />
      <button type="button" onClick={() => setCommitted("extern")}>
        extern
      </button>
      <output>{committed}</output>
    </>
  );
}

const typer = () => userEvent.setup({ advanceTimers: vi.advanceTimersByTime });

beforeEach(() => {
  vi.useFakeTimers();
  // Testing Library waits for the fake clock only through `jest.advanceTimersByTime`.
  vi.stubGlobal("jest", { advanceTimersByTime: (ms: number) => vi.advanceTimersByTime(ms) });
  commit.mockClear();
});
afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("useDebouncedText", () => {
  it("commits once, 300 ms after the last keystroke", async () => {
    const user = typer();
    render(<Harness />);
    await user.type(screen.getByLabelText("Suche"), "Festzelt");
    expect(commit).not.toHaveBeenCalled();
    expect(screen.getByLabelText("Suche")).toHaveValue("Festzelt");
    act(() => vi.advanceTimersByTime(299));
    expect(commit).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(1));
    expect(commit).toHaveBeenCalledTimes(1);
    expect(commit).toHaveBeenCalledWith("Festzelt");
  });

  it("commits at once on Enter, on blur and when the text is cleared", async () => {
    const user = typer();
    render(<Harness />);
    const input = screen.getByLabelText("Suche");
    await user.type(input, "a{Enter}");
    expect(commit).toHaveBeenLastCalledWith("a");
    await user.type(input, "b");
    await user.tab();
    expect(commit).toHaveBeenLastCalledWith("ab");
    await user.clear(input);
    expect(commit).toHaveBeenLastCalledWith("");
    expect(commit).toHaveBeenCalledTimes(3);
  });

  it("does not take the answer of its own commit over the newer draft", async () => {
    const user = typer();
    render(<Harness />);
    const input = screen.getByLabelText("Suche");
    await user.type(input, "ab");
    act(() => vi.advanceTimersByTime(300));
    await user.type(input, "c");
    expect(input).toHaveValue("abc");
  });

  it("takes a change from outside into the draft and drops the pending commit", async () => {
    const user = typer();
    render(<Harness />);
    const input = screen.getByLabelText("Suche");
    await user.type(input, "abc");
    await user.click(screen.getByRole("button", { name: "extern" }));
    commit.mockClear();
    act(() => vi.advanceTimersByTime(1000));
    expect(input).toHaveValue("extern");
    expect(commit).not.toHaveBeenCalled();
  });
});
