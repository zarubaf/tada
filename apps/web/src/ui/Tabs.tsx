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
  /** The `TabPanel` elements, one for each tab. */
  children: ReactNode;
}

/**
 * Tabs for two to six views of one list or record (doc/design/components.md). Arrow keys move
 * between the tabs; one tab is a tab stop. A type filter without panels is a `TypeFilter`.
 */
export function Tabs({ label, tabs, value, onChange, children }: TabsProps) {
  return (
    <AriaTabs
      className={styles.tabs}
      selectedKey={value}
      onSelectionChange={(key) => onChange(String(key))}
    >
      <TabList className={styles.list} aria-label={label}>
        {tabs.map((tab) => (
          <Tab key={tab.id} id={tab.id} className={styles.tab}>
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
