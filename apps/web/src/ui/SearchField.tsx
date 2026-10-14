import { IconSearch, IconX } from "@tabler/icons-react";
import { SearchField as AriaSearchField, Button, Input } from "react-aria-components";
import { t } from "../i18n";
import styles from "./SearchField.module.css";

export interface SearchFieldProps {
  /** The accessible name. The field has no visible label: its icon says what it is. */
  label: string;
  /** What to type, for example „Titel oder ID“. The label stays the accessible name. */
  placeholder?: string;
  value: string;
  onChange: (value: string) => void;
}

/**
 * A search field for the toolbar of a list (doc/design/components.md): an icon, the input and a
 * button that clears it. The caller filters; the field only holds the text.
 */
export function SearchField({ label, placeholder, value, onChange }: SearchFieldProps) {
  return (
    <AriaSearchField className={styles.search} aria-label={label} value={value} onChange={onChange}>
      <IconSearch className={styles.icon} size={14} stroke={1.5} aria-hidden="true" />
      <Input className={styles.input} placeholder={placeholder} />
      <Button className={styles.clear} aria-label={t("search-field-clear")}>
        <IconX size={14} stroke={1.5} aria-hidden="true" />
      </Button>
    </AriaSearchField>
  );
}
