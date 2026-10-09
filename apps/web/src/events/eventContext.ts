import { createContext, useCallback, useContext, useEffect, useState } from "react";
import { type Api, type Event, type EventProfile, type Field, problemMessage } from "../api/client";

/** What the event pages know about the facts: the profile and the field catalog behind it. */
export type ProfileState =
  | { kind: "loading" }
  | { kind: "failed"; message: string; requestId: string | undefined }
  | { kind: "loaded"; profile: EventProfile; fields: Field[] };

export interface EventContextValue {
  event: Event;
  profile: ProfileState;
  /** Loads the profile again. Resolves to true when it loaded. */
  reloadProfile: () => Promise<boolean>;
}

export const EventContext = createContext<EventContextValue | undefined>(undefined);

/** The event of the page and its profile. Only a page inside `EventPage` can call it. */
export function useEventContext(): EventContextValue {
  const value = useContext(EventContext);
  if (!value) {
    throw new Error("useEventContext needs an EventPage around it");
  }
  return value;
}

/** Loads the profile and the field catalog of an event. */
export function useEventProfile(
  api: Api,
  eventId: string,
): { profile: ProfileState; reloadProfile: () => Promise<boolean> } {
  const [profile, setProfile] = useState<ProfileState>({ kind: "loading" });

  const reloadProfile = useCallback(async () => {
    const failed = (error: Parameters<typeof problemMessage>[0]): ProfileState => ({
      kind: "failed",
      message: problemMessage(error),
      requestId: error?.request_id,
    });
    setProfile({ kind: "loading" });
    try {
      const path = { event_id: eventId };
      const [profileResult, fieldsResult] = await Promise.all([
        api.GET("/api/v1/events/{event_id}/profile", { params: { path } }),
        api.GET("/api/v1/events/{event_id}/fields", { params: { path } }),
      ]);
      if (profileResult.data && fieldsResult.data) {
        setProfile({
          kind: "loaded",
          profile: profileResult.data,
          fields: fieldsResult.data.items,
        });
        return true;
      }
      setProfile(failed(profileResult.error ?? fieldsResult.error));
    } catch {
      setProfile(failed(undefined));
    }
    return false;
  }, [api, eventId]);

  useEffect(() => {
    void reloadProfile();
  }, [reloadProfile]);

  return { profile, reloadProfile };
}
