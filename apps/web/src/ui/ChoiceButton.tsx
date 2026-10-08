import { Button as AriaButton } from "react-aria-components";
import styles from "./ChoiceButton.module.css";

export interface ChoiceButtonProps {
  title: string;
  /** A second line, for example a role. */
  detail: string;
  onPress: () => void;
  isPending?: boolean;
  isDisabled?: boolean;
}

/** A full-width button for one option of a choice, with a title and a detail line. */
export function ChoiceButton({ title, detail, onPress, isPending, isDisabled }: ChoiceButtonProps) {
  return (
    <AriaButton
      className={styles.choice}
      onPress={onPress}
      isPending={isPending}
      isDisabled={isDisabled}
    >
      <span className={styles.title}>{title}</span>
      <span className={styles.detail}>{detail}</span>
    </AriaButton>
  );
}
