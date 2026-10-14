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
  /** An icon before the label. It is decoration: the label names the tab. */
  icon?: ReactNode;
}

export interface TabsProps {
  /** The accessible name of the tab list. */
  label: string;
  tabs: TabItem[];
  /** The ID of the selected tab. The page owns it, for example in the URL. */
  value: string;
  onChange: (id: string) => void;
  /** `segmented` is a compact switch of views for a toolbar row; the default is the underlined row. */
  variant?: "underline" | "segmented";
  /** The tabs and the panel fill the height of their parent, which has to be a column. */
  fill?: boolean;
  /**
   * Places the tab list, for example in the toolbar row of a page (`PageToolbar`). The list has to
   * stay inside `Tabs`, so the caller wraps it here and not around the tabs. Without it, the list
   * stands alone above the panels.
   */
  renderList?: (list: ReactNode) => ReactNode;
  /** The `TabPanel` elements, one for each tab. */
  children: ReactNode;
}

/**
 * Tabs for two to six views of one list or record (doc/design/components.md). Arrow keys move
 * between the tabs; one tab is a tab stop. A type filter without panels is a `TypeFilter`.
 */
export function Tabs({
  label,
  tabs,
  value,
  onChange,
  variant = "underline",
  fill,
  renderList,
  children,
}: TabsProps) {
  const list = (
    <TabList className={styles.list} data-variant={variant} aria-label={label}>
      {tabs.map((tab) => (
        <Tab key={tab.id} id={tab.id} className={styles.tab}>
          {tab.icon}
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
  );
  return (
    <AriaTabs
      className={styles.tabs}
      {...(fill && { "data-fill": "" })}
      selectedKey={value}
      onSelectionChange={(key) => onChange(String(key))}
    >
      {renderList ? renderList(list) : list}
      {children}
    </AriaTabs>
  );
}

/** The content of one tab. Its `id` is the ID of the tab. */
export function TabPanel(props: Omit<TabPanelProps, "className" | "style">) {
  return <AriaTabPanel {...props} className={styles.panel} />;
}
