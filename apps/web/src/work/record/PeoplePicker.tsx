// A picker for several people (ADR 0082): a search field over a list of results, where a press
// selects or takes back a person, and the chosen people as a list of their own. It is the control
// `people` of the panel and of the dialogs that ask for the parties of a decision. The component
// filters nothing: the caller searches, for example on the server, and gives the results.
import { IconCheck, IconX } from "@tabler/icons-react";
import { type KeyboardEvent, useEffect, useId, useRef, useState } from "react";
import { t } from "../../i18n";
import { Avatar } from "../../ui/Avatar";
import { IconButton } from "../../ui/IconButton";
import { LiveRegion } from "../../ui/LiveRegion";
import styles from "./PeoplePicker.module.css";

/** The most people that one field names, as the contract of a decision allows. */
export const MAX_PEOPLE = 50;

export interface Person {
  id: string;
  /** The name, with the readable ID where the name alone is not unique. */
  label: string;
}

export interface PeoplePickerProps {
  /** The name of the field, for example „Beteiligte“. It names the search, the list and the chosen people. */
  label: string;
  /** The results for the typed text. A chosen person that is not among them stays chosen. */
  options: Person[];
  value: Person[];
  onChange: (value: Person[]) => void;
  /** The typed text, which the caller uses to search. */
  search: string;
  onSearch: (text: string) => void;
  /** The most people to choose. */
  max?: number;
  /** The search runs: the list says so instead of „no result“. */
  loading?: boolean;
  /** The message of a failed search. */
  failure?: string | undefined;
  /** The error of the field. It marks the search as invalid. */
  error?: string | undefined;
  /** The words in the empty search field. */
  placeholder?: string;
}

/**
 * The search is a combobox over a list that stays open. The list allows several choices: a press or
 * Enter on the active option toggles it, and the chosen people stay in a list above the search. Focus
 * stays in the search, so that the member goes on typing. The count is told with each change.
 */
export function PeoplePicker({
  label,
  options,
  value,
  onChange,
  search,
  onSearch,
  max = MAX_PEOPLE,
  loading = false,
  failure,
  error,
  placeholder,
}: PeoplePickerProps) {
  const id = useId();
  const input = useRef<HTMLInputElement>(null);
  const [active, setActive] = useState(-1);
  const [announcement, setAnnouncement] = useState("");
  const chosen = new Set(value.map((person) => person.id));
  const full = value.length >= max;
  const current = active < options.length ? active : -1;
  const optionId = (person: Person) => `${id}-option-${person.id}`;
  const activeId = options[current] ? optionId(options[current]) : undefined;
  const errorId = `${id}-error`;
  const countId = `${id}-count`;

  // The active option stays in view when the keys move it.
  useEffect(() => {
    if (activeId) {
      document.getElementById(activeId)?.scrollIntoView?.({ block: "nearest" });
    }
  }, [activeId]);

  const count = (people: Person[]) => t("people-picker-count", { count: people.length, max });

  const toggle = (person: Person) => {
    if (chosen.has(person.id)) {
      const next = value.filter((other) => other.id !== person.id);
      onChange(next);
      setAnnouncement(`${t("parties-removed", { name: person.label })}. ${count(next)}.`);
    } else if (!full) {
      const next = [...value, person];
      onChange(next);
      setAnnouncement(`${t("parties-added", { name: person.label })}. ${count(next)}.`);
    }
  };

  const onKeyDown = (event: KeyboardEvent) => {
    const last = options.length - 1;
    const go = (index: number) => {
      event.preventDefault();
      setActive(Math.max(0, Math.min(last, index)));
    };
    switch (event.key) {
      case "ArrowDown":
        go(current + 1);
        break;
      case "ArrowUp":
        go(current - 1);
        break;
      case "Home":
        if (options.length > 0) {
          go(0);
        }
        break;
      case "End":
        if (options.length > 0) {
          go(last);
        }
        break;
      case "Enter": {
        // Enter in the search must not send the form that holds the picker.
        event.preventDefault();
        const person = options[current];
        if (person) {
          toggle(person);
        }
        break;
      }
      case "Escape":
        // The first Escape empties the search; the next one closes what holds the picker.
        if (search !== "") {
          event.preventDefault();
          event.stopPropagation();
          onSearch("");
        }
        break;
    }
  };

  const describedBy = [error && errorId, value.length > 0 && countId].filter(Boolean).join(" ");

  return (
    <div className={styles.picker} data-invalid={error ? "" : undefined}>
      <label className={styles.label} htmlFor={`${id}-search`}>
        {label}
      </label>
      {value.length > 0 && (
        <ul className={styles.chosen} aria-label={t("people-picker-chosen", { label })}>
          {value.map((person) => (
            <li key={person.id} className={styles.person}>
              <Avatar name={person.label} id={person.id} decorative />
              <span className={styles.name} title={person.label}>
                {person.label}
              </span>
              <IconButton
                icon={IconX}
                size="sm"
                label={t("parties-remove", { name: person.label })}
                onPress={() => {
                  // The pressed button leaves with its row: focus goes back to the search.
                  input.current?.focus();
                  toggle(person);
                }}
              />
            </li>
          ))}
        </ul>
      )}
      <input
        ref={input}
        id={`${id}-search`}
        className={styles.search}
        type="text"
        role="combobox"
        aria-expanded="true"
        aria-controls={`${id}-list`}
        aria-autocomplete="list"
        aria-activedescendant={activeId}
        aria-invalid={error ? true : undefined}
        aria-describedby={describedBy || undefined}
        autoComplete="off"
        value={search}
        placeholder={placeholder ?? t("parties-search")}
        onChange={(event) => {
          setActive(-1);
          onSearch(event.target.value);
        }}
        onKeyDown={onKeyDown}
      />
      <div
        id={`${id}-list`}
        className={styles.list}
        role="listbox"
        aria-multiselectable="true"
        aria-label={t("people-picker-results", { label })}
      >
        {options.map((person, index) => {
          const selected = chosen.has(person.id);
          const disabled = full && !selected;
          return (
            // biome-ignore lint/a11y/useFocusableInteractive: the options are reached by the active descendant of the search
            // biome-ignore lint/a11y/useKeyWithClickEvents: the keys belong to the search that controls the list
            <div
              key={person.id}
              id={optionId(person)}
              className={styles.option}
              role="option"
              aria-selected={selected}
              aria-disabled={disabled || undefined}
              data-active={index === current || undefined}
              // The search keeps focus when a pointer presses an option.
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => !disabled && toggle(person)}
            >
              <Avatar name={person.label} id={person.id} decorative />
              <span className={styles.name} title={person.label}>
                {person.label}
              </span>
              {selected && <IconCheck size={14} aria-hidden="true" />}
            </div>
          );
        })}
      </div>
      {options.length === 0 && (
        <p className={styles.note}>{loading ? t("combobox-loading") : t("combobox-empty")}</p>
      )}
      {failure && <p className={styles.problem}>{failure}</p>}
      {value.length > 0 && (
        <p id={countId} className={styles.note}>
          {count(value)}
        </p>
      )}
      {full && <p className={styles.note}>{t("people-picker-full", { max })}</p>}
      {error && (
        <p id={errorId} className={styles.problem}>
          {error}
        </p>
      )}
      <LiveRegion kind="status" visuallyHidden>
        {announcement}
      </LiveRegion>
    </div>
  );
}
