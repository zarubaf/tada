import {
  ComboBox as AriaComboBox,
  FieldError,
  Input,
  Label,
  ListBox,
  ListBoxItem,
  Popover,
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
  /** The options load: the list says so instead of „no result“. */
  isLoading?: boolean;
  /** The error below the input. It marks the input as invalid. */
  error?: string | undefined;
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
  isLoading,
  error,
}: ComboBoxProps) {
  return (
    <AriaComboBox
      className={styles.combo}
      items={options}
      inputValue={inputValue}
      onInputChange={onInputChange}
      selectedKey={selectedKey ?? null}
      onSelectionChange={(key) => {
        onSelectionChange(key === null ? undefined : String(key));
        // The input shows the chosen option; a controlled input does not follow by itself.
        const chosen = options.find((option) => option.id === key);
        if (chosen) {
          onInputChange(chosen.label);
        }
      }}
      menuTrigger="focus"
      allowsEmptyCollection
      isInvalid={error !== undefined || undefined}
    >
      <Label className={styles.label}>{label}</Label>
      <Input className={styles.input} {...(placeholder === undefined ? {} : { placeholder })} />
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
