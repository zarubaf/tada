import { lazy, Suspense } from "react";
import type { Api, Workstream } from "../api/client";
import { Board, type BoardProps } from "./Board";
import { BoardSkeleton } from "./BoardSkeleton";
import type { WorkFilters } from "./workFilters";
import type { WorkView } from "./workView";

// The table and the timeline load on demand, so that the board does not pay for them (ADR 0082).
const WorkTable = lazy(() => import("./WorkTable"));
const WorkTimeline = lazy(() => import("./WorkTimeline"));

export interface WorkViewsProps extends BoardProps {
  view: WorkView;
  /** What the timeline view needs beyond the board: the filters, the areas and the event. */
  timeline: {
    api: Api;
    filters: WorkFilters;
    workstreams: Workstream[];
    /** The time zone of the event, so that „heute“ is its day. */
    timeZone: string | undefined;
    onReset: () => void;
  };
}

/** The view of the „Arbeit“ page that the tab names. Only the board is in the first bundle. */
export function WorkViews({ view, timeline, ...board }: WorkViewsProps) {
  if (view === "board") {
    return <Board {...board} />;
  }
  return (
    <Suspense fallback={<BoardSkeleton narrow={board.narrow} />}>
      {view === "table" ? (
        <WorkTable {...board} />
      ) : (
        <WorkTimeline
          items={board.items}
          eventId={board.eventId}
          api={timeline.api}
          filters={timeline.filters}
          workstreams={timeline.workstreams}
          timeZone={timeline.timeZone}
          onReset={timeline.onReset}
        />
      )}
    </Suspense>
  );
}
