import { Switch as AriaSwitch } from "react-aria-components";
import styles from "./Switch.module.css";

export interface SwitchProps {
  label: string;
  isSelected: boolean;
  onChange: (selected: boolean) => void;
  /** The member cannot change the setting, for example without the right. */
  isDisabled?: boolean;
  /** A request runs: a press does nothing, and the switch keeps focus (unlike a disabled one). */
  isPending?: boolean;
}

/** A switch for a setting with an immediate effect (doc/design/components.md). */
export function Switch({ label, isSelected, onChange, isDisabled, isPending }: SwitchProps) {
  return (
    <AriaSwitch
      className={styles.switch}
      isSelected={isSelected}
      onChange={(selected) => !isPending && onChange(selected)}
      isDisabled={isDisabled}
      data-pending={isPending || undefined}
    >
      <span className={styles.track} aria-hidden="true">
        <span className={styles.thumb} />
      </span>
      {label}
    </AriaSwitch>
  );
}
