import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import * as r from "../test/workRecords";
import { useBulkChange } from "./useBulkChange";
import { itemKey } from "./useWorkSelection";
import type { CommandResult, WorkCommands } from "./workCommands";
import { toWorkItem, type WorkItem } from "./workItem";

const make = (id: string) =>
  toWorkItem("action", r.action(id, { next_statuses: ["done"] }) as never, () => "") as WorkItem;
const ITEMS = [make("ACT-001"), make("ACT-002")];
const DONE = { field: "status", status: "done" } as const;

const ok: CommandResult<unknown> = { ok: true, record: {} };
const conflict: CommandResult<unknown> = {
  ok: false,
  failure: { reason: "conflict", message: "" },
};

function setup(changeStatus: (target: { id: string }) => Promise<CommandResult<unknown>>) {
  const commands = { action: { changeStatus } } as unknown as WorkCommands;
  const refresh = vi.fn(async () => true);
  const onFinished = vi.fn();
  const hook = renderHook(
    ({ scope }) => useBulkChange({ scope, commands, refresh, items: ITEMS, onFinished }),
    { initialProps: { scope: "e1" } },
  );
  return { hook, refresh, onFinished };
}

describe("useBulkChange", () => {
  it("loads a kind with a conflict again silently, with the refresh of the page", async () => {
    const { hook, refresh, onFinished } = setup(async ({ id }) =>
      id === ITEMS[0]?.id ? conflict : ok,
    );
    await act(async () => {
      hook.result.current.run(ITEMS, DONE);
    });
    await waitFor(() => expect(refresh).toHaveBeenCalledWith("action"));
    expect(onFinished).toHaveBeenCalledOnce();
  });

  it("is idle again when a command throws, and says what happened", async () => {
    let throws = true;
    const { hook } = setup(async () => {
      if (throws) {
        throw new Error("boom");
      }
      return ok;
    });
    await act(async () => {
      hook.result.current.run(ITEMS, DONE);
    });
    await waitFor(() => expect(hook.result.current.state.progress).toBeUndefined());
    throws = false;
    let outcome: string | undefined;
    await act(async () => {
      outcome = hook.result.current.run(ITEMS, DONE);
    });
    expect(outcome).toBe("started");
  });

  it("changes nothing for a run while one runs, and „unchanged“ when nothing is left to do", async () => {
    const gates: ((result: CommandResult<unknown>) => void)[] = [];
    const { hook } = setup(() => new Promise((resolve) => gates.push(resolve)));
    let first: string | undefined;
    let second: string | undefined;
    act(() => {
      first = hook.result.current.run(ITEMS, DONE);
      second = hook.result.current.run(ITEMS, DONE);
    });
    expect(first).toBe("started");
    expect(second).toBe("unchanged");
    await act(async () => {
      for (const gate of gates) {
        gate(ok);
      }
    });
    await waitFor(() => expect(hook.result.current.state.progress).toBeUndefined());
    let none: string | undefined;
    act(() => {
      none = hook.result.current.run(ITEMS, { field: "owner", userId: ITEMS[0]?.owner.id ?? "" });
    });
    expect(none).toBe("unchanged");
  });

  it("is busy while the kinds load again after a conflict", async () => {
    let finish: (loaded: boolean) => void = () => {};
    const commands = {
      action: { changeStatus: async () => conflict },
    } as unknown as WorkCommands;
    const refresh = vi.fn(() => new Promise<boolean>((resolve) => (finish = resolve)));
    const { result } = renderHook(() =>
      useBulkChange({ scope: "e1", commands, refresh, items: ITEMS, onFinished: vi.fn() }),
    );
    await act(async () => {
      result.current.run(ITEMS, DONE);
    });
    await waitFor(() => expect(result.current.state.reloading).toBe(true));
    let outcome: string | undefined;
    act(() => {
      outcome = result.current.run(ITEMS, DONE);
    });
    expect(outcome).toBe("unchanged");
    await act(async () => finish(true));
    await waitFor(() => expect(result.current.state.reloading).toBe(false));
  });

  it("drops the result of a run that belongs to another event", async () => {
    const gates: ((result: CommandResult<unknown>) => void)[] = [];
    const { hook, onFinished } = setup(() => new Promise((resolve) => gates.push(resolve)));
    act(() => {
      hook.result.current.run(ITEMS, DONE);
    });
    hook.rerender({ scope: "e2" });
    expect(hook.result.current.state.progress).toBeUndefined();
    await act(async () => {
      for (const gate of gates) {
        gate(conflict);
      }
    });
    expect(onFinished).not.toHaveBeenCalled();
    expect(hook.result.current.state.failures).toEqual([]);
  });

  it("keeps the failures on screen while a retry runs", async () => {
    let answer: CommandResult<unknown> = conflict;
    let release: () => void = () => {};
    const { hook } = setup(
      () =>
        new Promise((resolve) => {
          release = () => resolve(answer);
          if (answer === conflict) {
            resolve(answer);
          }
        }),
    );
    await act(async () => {
      hook.result.current.run(ITEMS, DONE);
    });
    await waitFor(() => expect(hook.result.current.state.reloading).toBe(false));
    expect(hook.result.current.state.failures).toHaveLength(2);
    answer = ok;
    act(() => hook.result.current.retry(new Set(ITEMS.map(itemKey))));
    expect(hook.result.current.state.progress).toBeDefined();
    expect(hook.result.current.state.failures).toHaveLength(2);
    await act(async () => release());
  });
});
