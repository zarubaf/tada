import { IconColumns3, IconTable, IconTimeline } from "@tabler/icons-react";
import type { ReactNode } from "react";
import { t } from "../i18n";
import { PageToolbar } from "../ui/PageToolbar";
import { TabPanel, Tabs } from "../ui/Tabs";
import type { WorkToolbarSlots } from "./WorkToolbar";
import styles from "./WorkViewTabs.module.css";
import { WORK_VIEWS, type WorkView } from "./workView";

export interface WorkViewTabsProps {
  view: WorkView;
  onChange: (view: WorkView) => void;
  /** The title, the filters and the like: they share one row with the view switch. */
  toolbar: WorkToolbarSlots;
  /** The banners and the view; they sit in the panel of the chosen tab. */
  children: ReactNode;
}

const VIEW_ICONS = { board: IconColumns3, table: IconTable, timeline: IconTimeline } as const;

function ViewIcon({ view }: { view: WorkView }) {
  const Icon = VIEW_ICONS[view];
  return <Icon size={14} stroke={1.5} aria-hidden="true" />;
}

/**
 * The view switch of the „Arbeit“ page: Board, Tabelle and Zeitplan (ADR 0082). It sits at the
 * right of the toolbar row. The filters are the same in every view, so the tab only changes what
 * shows below the row.
 */
export function WorkViewTabs({ view, onChange, toolbar, children }: WorkViewTabsProps) {
  return (
    <Tabs
      label={t("work-view-label")}
      tabs={WORK_VIEWS.map((id) => ({
        id,
        label: t(`work-view-${id}`),
        icon: <ViewIcon view={id} />,
      }))}
      value={view}
      variant="segmented"
      fill
      renderList={(list) => (
        <PageToolbar>
          {toolbar.lead}
          <div className={styles.end}>
            {list}
            {toolbar.end}
          </div>
          {toolbar.below}
        </PageToolbar>
      )}
      onChange={(id) => onChange(WORK_VIEWS.find((candidate) => candidate === id) ?? "board")}
    >
      <TabPanel id={view}>
        <div className={styles.content}>{children}</div>
      </TabPanel>
    </Tabs>
  );
}
