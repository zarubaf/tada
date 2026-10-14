import type { ReactNode } from "react";
import { t } from "../i18n";
import { TabPanel, Tabs } from "../ui/Tabs";
import { WORK_VIEWS, type WorkView } from "./workView";

export interface WorkViewTabsProps {
  view: WorkView;
  onChange: (view: WorkView) => void;
  /** The toolbar and the view; they sit in the panel of the chosen tab. */
  children: ReactNode;
}

/**
 * The view tabs of the „Arbeit“ page: Board, Tabelle and Zeitplan (ADR 0082). The filters are the
 * same in every view, so the toolbar sits in the panel and the tab only changes what shows below.
 */
export function WorkViewTabs({ view, onChange, children }: WorkViewTabsProps) {
  return (
    <Tabs
      label={t("work-view-label")}
      tabs={WORK_VIEWS.map((id) => ({ id, label: t(`work-view-${id}`) }))}
      value={view}
      onChange={(id) => onChange(WORK_VIEWS.find((candidate) => candidate === id) ?? "board")}
    >
      <TabPanel id={view}>{children}</TabPanel>
    </Tabs>
  );
}
