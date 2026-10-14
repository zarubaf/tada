import type { ReactNode } from "react";
import { ToggleButton, ToggleButtonGroup } from "react-aria-components";
import styles from "./TypeFilter.module.css";

export interface TypeFilterOption {
  id: string;
  label: string;
  /** A number after the label, for example the open tasks of a type. */
  count?: number;
  /** An icon before the label. It is decoration: the label names the option. */
  icon?: ReactNode;
}

export interface TypeFilterProps {
  /** The accessible name of the group. */
  label: string;
  options: TypeFilterOption[];
  /** The ID of the chosen option. The page owns it, for example in the URL. */
  value: string;
  onChange: (id: string) => void;
}

/**
 * A row of pills that filters a list by type, with counts (doc/design/components.md). One option
 * is always chosen. It has no panels, so it is a group of toggle buttons and not a tab list.
 */
export function TypeFilter({ label, options, value, onChange }: TypeFilterProps) {
  return (
    <ToggleButtonGroup
      className={styles.group}
      aria-label={label}
      selectionMode="single"
      disallowEmptySelection
      selectedKeys={[value]}
      onSelectionChange={(keys) => {
        const [key] = [...keys];
        if (key !== undefined) {
          onChange(String(key));
        }
      }}
    >
      {options.map((option) => (
        <ToggleButton key={option.id} id={option.id} className={styles.option}>
          {option.icon}
          {option.label}
          {option.count !== undefined && (
            <>
              {" "}
              <span className={styles.count}>{option.count}</span>
            </>
          )}
        </ToggleButton>
      ))}
    </ToggleButtonGroup>
  );
}
