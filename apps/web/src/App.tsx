import { lazy, Suspense } from "react";
import { type Api, createApi } from "./api/client";
import { CreateEventForm } from "./events/CreateEventForm";
import { EventMembersPage } from "./events/EventMembersPage";
import { EventPage } from "./events/EventPage";
import { EventsPage } from "./events/EventsPage";
import { t } from "./i18n";
import { MembersPage } from "./members/MembersPage";
import { NotFoundPage } from "./NotFoundPage";
import { Redirect, Route, Router, Routes } from "./router/Router";
import { ChooseOrganizationPage } from "./session/ChooseOrganizationPage";
import { SessionProvider } from "./session/SessionProvider";
import { SettingsLayout } from "./settings/SettingsLayout";
import { Shell } from "./shell/Shell";
import { InvitationPage } from "./sign-in/InvitationPage";
import { MagicLinkPage } from "./sign-in/MagicLinkPage";
import { SignInPage } from "./sign-in/SignInPage";
import { TelegramPage } from "./telegram/TelegramPage";
import { SkipLink } from "./ui/SkipLink";

const defaultApi = createApi();

// The component gallery exists only in builds that are not production builds (ADR 0024).
const Gallery =
  import.meta.env.MODE === "production"
    ? null
    : lazy(() => import("./gallery/Gallery").then((module) => ({ default: module.Gallery })));

export function App({ api = defaultApi }: { api?: Api }) {
  return (
    <Router>
      <SkipLink />
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
                {/* The pages of the sign-in */}
                <Route path="/sign-in">
                  <SignInPage api={api} />
                </Route>
                <Route path="/sign-in/link">
                  <MagicLinkPage api={api} />
                </Route>
                <Route path="/invitation">
                  <InvitationPage api={api} />
                </Route>
                <Route path="/choose-organization">
                  <ChooseOrganizationPage api={api} />
                </Route>
                {/* Event screens (Task 21) */}
                <Route path="/events/new">
                  <CreateEventForm api={api} />
                </Route>
                <Route path="/events/:eventId">
                  <EventPage api={api}>
                    <p>{t("event-overview-placeholder")}</p>
                  </EventPage>
                </Route>
                <Route path="/events/:eventId/members">
                  <EventPage api={api}>
                    <EventMembersPage api={api} />
                  </EventPage>
                </Route>
                {/* Settings (Task 20) */}
                <Route path="/settings/members">
                  <SettingsLayout>
                    <MembersPage api={api} />
                  </SettingsLayout>
                </Route>
                <Route path="/settings/telegram">
                  <SettingsLayout>
                    <TelegramPage api={api} />
                  </SettingsLayout>
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
