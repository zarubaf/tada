import { lazy, Suspense } from "react";
import { type Api, createApi } from "./api/client";
import { DocumentPage } from "./documents/DocumentPage";
import { DocumentsPage } from "./documents/DocumentsPage";
import { CreateEventForm } from "./events/CreateEventForm";
import { EventMembersPage } from "./events/EventMembersPage";
import { EventOverview } from "./events/EventOverview";
import { EventPage } from "./events/EventPage";
import { EventsPage } from "./events/EventsPage";
import { MembersPage } from "./members/MembersPage";
import { NotFoundPage } from "./NotFoundPage";
import { InboxProvider } from "./review/InboxProvider";
import { ReviewInbox } from "./review/ReviewInbox";
import { PrivacyPage } from "./privacy/PrivacyPage";
import { Redirect, Route, Router, Routes } from "./router/Router";
import { ChooseOrganizationPage } from "./session/ChooseOrganizationPage";
import { SessionProvider } from "./session/SessionProvider";
import { OrganizationPage } from "./settings/OrganizationPage";
import { SettingsLayout } from "./settings/SettingsLayout";
import { Shell } from "./shell/Shell";
import { InvitationPage } from "./sign-in/InvitationPage";
import { MagicLinkPage } from "./sign-in/MagicLinkPage";
import { SignInPage } from "./sign-in/SignInPage";
import { TelegramPage } from "./telegram/TelegramPage";
import { TokensPage } from "./tokens/TokensPage";
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
            <InboxProvider api={api}>
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
                  {/* The event screens */}
                  <Route path="/events/new">
                    <CreateEventForm api={api} />
                  </Route>
                  <Route path="/events/:eventId">
                    <EventPage api={api}>
                      <EventOverview api={api} />
                    </EventPage>
                  </Route>
                  <Route path="/events/:eventId/members">
                    <EventPage api={api}>
                      <EventMembersPage api={api} />
                    </EventPage>
                  </Route>
                  <Route path="/events/:eventId/documents">
                    <EventPage api={api}>
                      <DocumentsPage api={api} />
                    </EventPage>
                  </Route>
                  <Route path="/documents/:documentId">
                    <DocumentPage api={api} />
                  </Route>
                  <Route path="/privacy">
                    <PrivacyPage api={api} />
                  </Route>
                  {/* The Review Inbox */}
                  <Route path="/inbox">
                    <ReviewInbox api={api} />
                  </Route>
                  <Route path="/inbox/:changesetId">
                    <ReviewInbox api={api} />
                  </Route>
                  {/* The settings */}
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
                  <Route path="/settings/tokens">
                    <SettingsLayout>
                      <TokensPage api={api} />
                    </SettingsLayout>
                  </Route>
                  <Route path="/settings/organization">
                    <SettingsLayout>
                      <OrganizationPage api={api} />
                    </SettingsLayout>
                  </Route>
                  <Route path="*">
                    <NotFoundPage />
                  </Route>
                </Routes>
              </Shell>
            </InboxProvider>
          </SessionProvider>
        </Route>
      </Routes>
    </Router>
  );
}
