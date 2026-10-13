import { type RefObject, useContext, useLayoutEffect, useRef } from "react";
import {
  ComboBox as AriaComboBox,
  ComboBoxStateContext,
  FieldError,
  Input,
  Label,
  ListBox,
  ListBoxItem,
  Popover,
  Text,
} from "react-aria-components";
import { t } from "../i18n";
import styles from "./ComboBox.module.css";
import popover from "./Popover.module.css";

export interface ComboBoxProps {
  label: string;
  /** The options that match the text. The caller filters them, for example on the server. */
  options: { id: string; label: string }[];
  /** The text of the input. */
  inputValue: string;
  onInputChange: (text: string) => void;
  /** The ID of the chosen option, or nothing. */
  selectedKey: string | undefined;
  onSelectionChange: (id: string | undefined) => void;
  placeholder?: string;
  /** Help below the input. */
  help?: string;
  /** Empties the input after a choice, for a list that collects the choices. */
  clearOnSelect?: boolean;
  /** The options load: the list says so instead of „no result“. */
  isLoading?: boolean;
  /** The error below the input. It marks the input as invalid. */
  error?: string | undefined;
}

/**
 * Closes the list that the empty input opens after a choice. React Aria opens the list when the
 * text changes while the input has focus, and the list has no prop that stops it.
 */
function CloseAfterChoice({ closing }: { closing: RefObject<boolean> }) {
  const state = useContext(ComboBoxStateContext);
  useLayoutEffect(() => {
    if (closing.current && state?.isOpen) {
      state.close();
    }
  });
  return null;
}

/**
 * A search in a list that is too long for a select (doc/design/components.md): the member types,
 * the caller loads the matching options, the member picks one. The component filters nothing.
 */
export function ComboBox({
  label,
  options,
  inputValue,
  onInputChange,
  selectedKey,
  onSelectionChange,
  placeholder,
  help,
  clearOnSelect,
  isLoading,
  error,
}: ComboBoxProps) {
  // True from a choice of a `clearOnSelect` box until the member types or opens the list again.
  const closing = useRef(false);
  return (
    <AriaComboBox
      className={styles.combo}
      items={options}
      inputValue={inputValue}
      onInputChange={(text) => {
        closing.current = false;
        onInputChange(text);
      }}
      onOpenChange={(open, trigger) => {
        if (open && trigger !== "input") {
          closing.current = false;
        }
      }}
      selectedKey={selectedKey ?? null}
      onSelectionChange={(key) => {
        closing.current = clearOnSelect === true && key !== null;
        onSelectionChange(key === null ? undefined : String(key));
        // The input shows the chosen option; a controlled input does not follow by itself.
        const chosen = options.find((option) => option.id === key);
        if (chosen) {
          // Not through the handler of the box: that one ends the closing.
          onInputChange(clearOnSelect ? "" : chosen.label);
        }
      }}
      menuTrigger="focus"
      allowsEmptyCollection
      isInvalid={error !== undefined || undefined}
    >
      <CloseAfterChoice closing={closing} />
      <Label className={styles.label}>{label}</Label>
      <Input className={styles.input} {...(placeholder === undefined ? {} : { placeholder })} />
      {help && (
        <Text slot="description" className={styles.help}>
          {help}
        </Text>
      )}
      {error && <FieldError className={styles.error}>{error}</FieldError>}
      <Popover className={popover.popover}>
        <ListBox
          className={popover.list}
          renderEmptyState={() => (
            <p className={styles.empty}>
              {isLoading ? t("combobox-loading") : t("combobox-empty")}
            </p>
          )}
        >
          {(option: { id: string; label: string }) => (
            <ListBoxItem id={option.id} textValue={option.label} className={popover.item}>
              {option.label}
            </ListBoxItem>
          )}
        </ListBox>
      </Popover>
    </AriaComboBox>
  );
}
