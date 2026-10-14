import type { ReactNode } from "react";
import {
  TabPanel as AriaTabPanel,
  Tabs as AriaTabs,
  Tab,
  TabList,
  type TabPanelProps,
} from "react-aria-components";
import styles from "./Tabs.module.css";

export interface TabItem {
  id: string;
  label: string;
  /** A number after the label, for example the open tasks of a type. */
  count?: number;
}

export interface TabsProps {
  /** The accessible name of the tab list. */
  label: string;
  tabs: TabItem[];
  /** The ID of the selected tab. The page owns it, for example in the URL. */
  value: string;
  onChange: (id: string) => void;
  /** `view` is an underline for the views of one record or list. `filter` is a row of pills. */
  look?: "view" | "filter";
  /** The `TabPanel` elements. Without them the tabs only report the choice. */
  children?: ReactNode;
}

/**
 * React Aria points the selected tab at a panel and has no prop to stop it. Without panels the
 * reference would dangle, so a tab list without panels removes it after each commit. The inline ref
 * callback is new on each render, so React calls it again after each commit.
 */
function dropPanelReference(tab: HTMLDivElement | null) {
  tab?.removeAttribute("aria-controls");
}

/**
 * Tabs for two to six views of one list or record, or a type filter with counts
 * (doc/design/components.md). Arrow keys move between the tabs; one tab is a tab stop.
 */
export function Tabs({ label, tabs, value, onChange, look = "view", children }: TabsProps) {
  return (
    <AriaTabs
      className={styles.tabs}
      selectedKey={value}
      onSelectionChange={(key) => onChange(String(key))}
    >
      <TabList className={styles.list} data-look={look} aria-label={label}>
        {tabs.map((tab) => (
          <Tab
            key={tab.id}
            id={tab.id}
            className={styles.tab}
            ref={children ? null : (tab) => dropPanelReference(tab)}
          >
            {tab.label}
            {tab.count !== undefined && (
              <>
                {" "}
                <span className={styles.count}>{tab.count}</span>
              </>
            )}
          </Tab>
        ))}
      </TabList>
      {children}
    </AriaTabs>
  );
}

/** The content of one tab. Its `id` is the ID of the tab. */
export function TabPanel(props: Omit<TabPanelProps, "className" | "style">) {
  return <AriaTabPanel {...props} className={styles.panel} />;
}
