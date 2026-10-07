import { lazy, Suspense } from "react";
import { type Api, createApi } from "./api/client";
import { EventsPage } from "./events/EventsPage";
import { NotFoundPage } from "./NotFoundPage";
import { Redirect, Route, Router, Routes } from "./router/Router";
import { SessionProvider } from "./session/SessionProvider";
import { Shell } from "./shell/Shell";

const defaultApi = createApi();

// The component gallery exists only in builds that are not production builds (ADR 0024).
const Gallery =
  import.meta.env.MODE === "production"
    ? null
    : lazy(() => import("./gallery/Gallery").then((module) => ({ default: module.Gallery })));

export function App({ api = defaultApi }: { api?: Api }) {
  return (
    <Router>
      <Routes>
        {Gallery && (
          <Route path="/_gallery">
            <Suspense>
              <Gallery />
            </Suspense>
          </Route>
        )}
        <Route path="*">
          <SessionProvider api={api}>
            <Shell api={api}>
              <Routes>
                <Route path="/">
                  <Redirect to="/events" />
                </Route>
                <Route path="/events">
                  <EventsPage api={api} />
                </Route>
                <Route path="*">
                  <NotFoundPage />
                </Route>
              </Routes>
            </Shell>
          </SessionProvider>
        </Route>
      </Routes>
    </Router>
  );
}
