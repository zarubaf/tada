// The selects that search persons and institutions on the server. They live apart from the
// directory-based selects of `fields.tsx`, which the review inbox loads at once.
import { useRef, useState } from "react";
import type { Api } from "../api/client";
import { t } from "../i18n";
import { Button } from "../ui/Button";
import { ComboBox } from "../ui/ComboBox";
import { useFocusAfterCommit } from "../ui/focus";
import type { Choice } from "./directory";
import { usePromisorSearch } from "./usePromisorSearch";
import styles from "./Work.module.css";

/**
 * A search for one person or institution of the organization. The server searches by name (ADR
 * 0069); the member types and picks. The picker keeps its own text. A parent that wants a fresh
 * picker, for example after a save, gives it a new `key`.
 */
export function PartyPicker({
  api,
  label,
  placeholder,
  help,
  error,
  exclude = [],
  resetOnChoice = false,
  onChange,
}: {
  api: Api;
  label: string;
  placeholder?: string;
  help?: string;
  error?: string | undefined;
  /** The IDs of parties that the search does not offer, for example those already chosen. */
  exclude?: string[];
  /** Empties the input after a choice, for a list that collects the choices. */
  resetOnChoice?: boolean;
  /** The chosen party (`person:{uuid}` or `institution:{uuid}`), or nothing when the member clears it. */
  onChange: (party: Choice | undefined) => void;
}) {
  const [text, setText] = useState("");
  const [chosen, setChosen] = useState<Choice>();
  // The search stops while the text shows the label of the chosen option.
  const search = usePromisorSearch(api, text, text !== chosen?.label);
  const offered = search.options.filter((option) => !exclude.includes(option.id));
  return (
    <ComboBox
      label={label}
      {...(placeholder !== undefined && { placeholder })}
      {...(help !== undefined && { help })}
      options={offered}
      inputValue={text}
      onInputChange={(next) => {
        setText(next);
        // React Aria keeps a choice while both the text and the key are controlled; a text that
        // the member emptied is no choice any more.
        if (next === "" && chosen !== undefined && !resetOnChoice) {
          setChosen(undefined);
          onChange(undefined);
        }
      }}
      selectedKey={resetOnChoice ? undefined : chosen?.id}
      clearOnSelect={resetOnChoice}
      onSelectionChange={(id) => {
        const party = offered.find((option) => option.id === id);
        if (!resetOnChoice) {
          setChosen(party);
        }
        onChange(party);
      }}
      isLoading={search.loading}
      error={error ?? search.failure}
    />
  );
}

/** The most parties that one decision names. */
export const MAX_PARTIES = 50;

/**
 * A list of persons and institutions that grows with a search: the parties that decided. Each
 * chosen party has a button that removes it; a party counts once.
 */
export function PartiesSelect({
  api,
  label,
  listLabel,
  value,
  onChange,
  onAnnounce,
  error,
}: {
  api: Api;
  label: string;
  /** The name of the list of chosen parties. */
  listLabel: string;
  /** The chosen parties; the ID of a choice is `person:{uuid}` or `institution:{uuid}`. */
  value: Choice[];
  onChange: (parties: Choice[]) => void;
  /**
   * Tells each choice and removal. The input empties and focus stays, so a screen reader hears
   * nothing else; the page owns the polite region (doc/design/accessibility.md).
   */
  onAnnounce: (message: string) => void;
  error?: string | undefined;
}) {
  const box = useRef<HTMLDivElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  return (
    <div ref={box} className={styles.parties}>
      <PartyPicker
        api={api}
        label={label}
        error={error}
        exclude={value.map((party) => party.id)}
        resetOnChoice
        onChange={(party) => {
          if (party !== undefined) {
            onChange([...value, party]);
            onAnnounce(t("parties-added", { name: party.label }));
          }
        }}
      />
      {value.length > 0 && (
        <ul aria-label={listLabel} className={styles.partyList}>
          {value.map((party) => (
            <li key={party.id} className={styles.party}>
              <span>{party.label}</span>
              <Button
                variant="quiet"
                aria-label={t("parties-remove", { name: party.label })}
                onPress={() => {
                  const index = value.indexOf(party);
                  onChange(value.filter((chosen) => chosen.id !== party.id));
                  onAnnounce(t("parties-removed", { name: party.label }));
                  // The pressed button leaves: focus goes to its neighbour, or to the search.
                  focusAfterCommit(() => {
                    const buttons = box.current?.querySelectorAll<HTMLElement>("ul button");
                    return (
                      buttons?.[Math.min(index, buttons.length - 1)] ??
                      box.current?.querySelector("input")
                    );
                  });
                }}
              >
                {t("parties-remove-short")}
              </Button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
