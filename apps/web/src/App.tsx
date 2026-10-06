import { lazy, Suspense } from "react";
import { createApi } from "./api/client";
import { EventsPage } from "./events/EventsPage";

const api = createApi();

// The component gallery exists only in builds that are not production builds (ADR 0024).
const Gallery =
  import.meta.env.MODE === "production"
    ? null
    : lazy(() => import("./gallery/Gallery").then((module) => ({ default: module.Gallery })));

export function App() {
  if (Gallery && window.location.pathname === "/_gallery") {
    return (
      <Suspense>
        <Gallery />
      </Suspense>
    );
  }
  return <EventsPage api={api} />;
}
