import { render, screen } from "@testing-library/react";
import { Suspense } from "react";
import { describe, expect, it } from "vitest";
import { useFragmentToken } from "./fragment";

/** Suspends once after it read the token, as React does when it throws away a render and retries. */
function Probe({ ready }: { ready: Promise<void> }) {
  const token = useFragmentToken();
  if (!settled.has(ready)) {
    throw ready.then(() => settled.add(ready));
  }
  return <p data-testid="token">{token ?? "none"}</p>;
}
const settled = new WeakSet<Promise<void>>();

describe("useFragmentToken", () => {
  it("still has the token when React throws a render away and renders again", async () => {
    window.history.replaceState(null, "", "/invitation#token=abc");
    render(
      <Suspense fallback={<p>wait</p>}>
        <Probe ready={Promise.resolve()} />
      </Suspense>,
    );
    expect((await screen.findByTestId("token")).textContent).toBe("abc");
  });

  it("removes the fragment from the address once the page is in", async () => {
    window.history.replaceState(null, "", "/invitation?x=1#token=abc");
    render(<Probe ready={settledPromise()} />);
    expect(window.location.hash).toBe("");
    expect(window.location.pathname + window.location.search).toBe("/invitation?x=1");
  });
});

function settledPromise(): Promise<void> {
  const done = Promise.resolve();
  settled.add(done);
  return done;
}
