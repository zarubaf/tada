import { lazy, type ReactNode } from "react";
import { type Api, createApi } from "./api/client";
import { EventPage } from "./events/EventPage";
import { RegisterNav } from "./events/RegisterNav";
import { NotFoundPage } from "./NotFoundPage";
import { InboxProvider } from "./review/InboxProvider";
import { Route, Router, Routes } from "./router/Router";
import { SessionProvider } from "./session/SessionProvider";
import { SettingsRoute } from "./settings/SettingsRoute";
import { Shell } from "./shell/Shell";
import { RouteSuspense } from "./ui/RouteSuspense";
import { SkipLink } from "./ui/SkipLink";
import { MyWorkPage } from "./work/MyWorkPage";

const defaultApi = createApi();

// The settings pages load on demand: a member opens them seldom, and the first load stays inside
// the bundle budget of ADR 0024.
const MembersPage = lazy(() =>
  import("./members/MembersPage").then((module) => ({ default: module.MembersPage })),
);
const TelegramPage = lazy(() =>
  import("./telegram/TelegramPage").then((module) => ({ default: module.TelegramPage })),
);
const TokensPage = lazy(() =>
  import("./tokens/TokensPage").then((module) => ({ default: module.TokensPage })),
);
const OrganizationPage = lazy(() =>
  import("./settings/OrganizationPage").then((module) => ({ default: module.OrganizationPage })),
);
// The registers of an event load on demand too: they bring the pickers of the work forms.
const ActionsPage = lazy(() =>
  import("./work/ActionsPage").then((module) => ({ default: module.ActionsPage })),
);
const CommitmentsPage = lazy(() =>
  import("./work/CommitmentsPage").then((module) => ({ default: module.CommitmentsPage })),
);
const DecisionsPage = lazy(() =>
  import("./work/DecisionsPage").then((module) => ({ default: module.DecisionsPage })),
);
const MilestonesPage = lazy(() =>
  import("./work/MilestonesPage").then((module) => ({ default: module.MilestonesPage })),
);
// The timeline is a large screen of its own and seldom the first one.
const TimelinePage = lazy(() =>
  import("./timeline/TimelinePage").then((module) => ({ default: module.TimelinePage })),
);
const WorkstreamsPage = lazy(() =>
  import("./work/WorkstreamsPage").then((module) => ({ default: module.WorkstreamsPage })),
);
// The register of persons and institutions, the event members and the diff of two document
// versions are seldom the first screen.
const PartiesPage = lazy(() =>
  import("./parties/PartiesPage").then((module) => ({ default: module.PartiesPage })),
);
const EventMembersPage = lazy(() =>
  import("./events/EventMembersPage").then((module) => ({ default: module.EventMembersPage })),
);
const DocumentDiffPage = lazy(() =>
  import("./documents/DocumentDiffPage").then((module) => ({ default: module.DocumentDiffPage })),
);
const PmPage = lazy(() =>
  import("./settings/PmPage").then((module) => ({ default: module.PmPage })),
);
const AccountPage = lazy(() =>
  import("./settings/AccountPage").then((module) => ({ default: module.AccountPage })),
);

// The other screens load on demand too: the first load stays inside the bundle budget of ADR 0024.
const DocumentPage = lazy(() =>
  import("./documents/DocumentPage").then((module) => ({ default: module.DocumentPage })),
);
const DocumentsPage = lazy(() =>
  import("./documents/DocumentsPage").then((module) => ({ default: module.DocumentsPage })),
);
const CreateEventForm = lazy(() =>
  import("./events/CreateEventForm").then((module) => ({ default: module.CreateEventForm })),
);
const EventOverview = lazy(() =>
  import("./events/EventOverview").then((module) => ({ default: module.EventOverview })),
);
const EventsPage = lazy(() =>
  import("./events/EventsPage").then((module) => ({ default: module.EventsPage })),
);
const PrivacyPage = lazy(() =>
  import("./privacy/PrivacyPage").then((module) => ({ default: module.PrivacyPage })),
);
const ReviewInbox = lazy(() =>
  import("./review/ReviewInbox").then((module) => ({ default: module.ReviewInbox })),
);
const ChooseOrganizationPage = lazy(() =>
  import("./session/ChooseOrganizationPage").then((module) => ({
    default: module.ChooseOrganizationPage,
  })),
);
const InvitationPage = lazy(() =>
  import("./sign-in/InvitationPage").then((module) => ({ default: module.InvitationPage })),
);
const MagicLinkPage = lazy(() =>
  import("./sign-in/MagicLinkPage").then((module) => ({ default: module.MagicLinkPage })),
);
const SignInPage = lazy(() =>
  import("./sign-in/SignInPage").then((module) => ({ default: module.SignInPage })),
);

// The component gallery exists only in builds that are not production builds (ADR 0024).
const Gallery =
  import.meta.env.MODE === "production"
    ? null
    : lazy(() => import("./gallery/Gallery").then((module) => ({ default: module.Gallery })));

/**
 * A register of an event: the second row of the event navigation stays while the register loads,
 * so that it does not jump when the member changes the register.
 */
function RegisterPage({ api, children }: { api: Api; children: ReactNode }) {
  return (
    <EventPage api={api}>
      <RegisterNav />
      <RouteSuspense>{children}</RouteSuspense>
    </EventPage>
  );
}

export function App({ api = defaultApi }: { api?: Api }) {
  return (
    <Router>
      <SkipLink />
      <Routes>
        {Gallery && (
          <Route path="/_gallery">
            <RouteSuspense>
              <Gallery />
            </RouteSuspense>
          </Route>
        )}
        <Route path="*">
          <SessionProvider api={api}>
            <InboxProvider api={api}>
              <Shell api={api}>
                <Routes>
                  <Route path="/">
                    <MyWorkPage api={api} />
                  </Route>
                  <Route path="/events">
                    <RouteSuspense>
                      <EventsPage api={api} />
                    </RouteSuspense>
                  </Route>
                  {/* The pages of the sign-in */}
                  <Route path="/sign-in">
                    <RouteSuspense>
                      <SignInPage api={api} />
                    </RouteSuspense>
                  </Route>
                  <Route path="/sign-in/link">
                    <RouteSuspense>
                      <MagicLinkPage api={api} />
                    </RouteSuspense>
                  </Route>
                  <Route path="/invitation">
                    <RouteSuspense>
                      <InvitationPage api={api} />
                    </RouteSuspense>
                  </Route>
                  <Route path="/choose-organization">
                    <RouteSuspense>
                      <ChooseOrganizationPage api={api} />
                    </RouteSuspense>
                  </Route>
                  {/* The event screens */}
                  <Route path="/events/new">
                    <RouteSuspense>
                      <CreateEventForm api={api} />
                    </RouteSuspense>
                  </Route>
                  <Route path="/events/:eventId">
                    <EventPage api={api}>
                      <RouteSuspense>
                        <EventOverview api={api} />
                      </RouteSuspense>
                    </EventPage>
                  </Route>
                  <Route path="/events/:eventId/members">
                    <EventPage api={api}>
                      <RouteSuspense>
                        <EventMembersPage api={api} />
                      </RouteSuspense>
                    </EventPage>
                  </Route>
                  <Route path="/events/:eventId/documents">
                    <EventPage api={api}>
                      <RouteSuspense>
                        <DocumentsPage api={api} />
                      </RouteSuspense>
                    </EventPage>
                  </Route>
                  <Route path="/events/:eventId/actions">
                    <EventPage api={api}>
                      <RouteSuspense>
                        <ActionsPage api={api} />
                      </RouteSuspense>
                    </EventPage>
                  </Route>
                  <Route path="/events/:eventId/timeline">
                    <EventPage api={api}>
                      <RouteSuspense>
                        <TimelinePage api={api} />
                      </RouteSuspense>
                    </EventPage>
                  </Route>
                  <Route path="/events/:eventId/commitments">
                    <RegisterPage api={api}>
                      <CommitmentsPage api={api} />
                    </RegisterPage>
                  </Route>
                  <Route path="/events/:eventId/milestones">
                    <RegisterPage api={api}>
                      <MilestonesPage api={api} />
                    </RegisterPage>
                  </Route>
                  <Route path="/events/:eventId/decisions">
                    <RegisterPage api={api}>
                      <DecisionsPage api={api} />
                    </RegisterPage>
                  </Route>
                  <Route path="/events/:eventId/workstreams">
                    <EventPage api={api}>
                      <RouteSuspense>
                        <WorkstreamsPage api={api} />
                      </RouteSuspense>
                    </EventPage>
                  </Route>
                  <Route path="/documents/:documentId">
                    <RouteSuspense>
                      <DocumentPage api={api} />
                    </RouteSuspense>
                  </Route>
                  <Route path="/documents/:documentId/diff">
                    <RouteSuspense>
                      <DocumentDiffPage api={api} />
                    </RouteSuspense>
                  </Route>
                  <Route path="/privacy">
                    <RouteSuspense>
                      <PrivacyPage api={api} />
                    </RouteSuspense>
                  </Route>
                  {/* The register of persons and institutions */}
                  <Route path="/persons">
                    <RouteSuspense>
                      <PartiesPage api={api} kind="person" />
                    </RouteSuspense>
                  </Route>
                  <Route path="/institutions">
                    <RouteSuspense>
                      <PartiesPage api={api} kind="institution" />
                    </RouteSuspense>
                  </Route>
                  {/* The Review Inbox */}
                  <Route path="/inbox">
                    <RouteSuspense>
                      <ReviewInbox api={api} />
                    </RouteSuspense>
                  </Route>
                  <Route path="/inbox/:changesetId">
                    <RouteSuspense>
                      <ReviewInbox api={api} />
                    </RouteSuspense>
                  </Route>
                  {/* The settings */}
                  <Route path="/settings/members">
                    <SettingsRoute>
                      <MembersPage api={api} />
                    </SettingsRoute>
                  </Route>
                  <Route path="/settings/telegram">
                    <SettingsRoute>
                      <TelegramPage api={api} />
                    </SettingsRoute>
                  </Route>
                  <Route path="/settings/tokens">
                    <SettingsRoute>
                      <TokensPage api={api} />
                    </SettingsRoute>
                  </Route>
                  <Route path="/settings/organization">
                    <SettingsRoute>
                      <OrganizationPage api={api} />
                    </SettingsRoute>
                  </Route>
                  <Route path="/settings/ai-pm">
                    <SettingsRoute>
                      <PmPage api={api} />
                    </SettingsRoute>
                  </Route>
                  <Route path="/settings/account">
                    <SettingsRoute>
                      <AccountPage api={api} />
                    </SettingsRoute>
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
