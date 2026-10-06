import { createApi } from "./api/client";
import { EventsPage } from "./events/EventsPage";

const api = createApi();

export function App() {
  return <EventsPage api={api} />;
}
