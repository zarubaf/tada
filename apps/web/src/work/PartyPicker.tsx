// The select that searches persons and institutions on the server. It lives apart from the
// directory-based selects of `fields.tsx`, which the review inbox loads at once.
import { useState } from "react";
import type { Api } from "../api/client";
import { ComboBox } from "../ui/ComboBox";
import type { Choice } from "./directory";
import { usePromisorSearch } from "./usePromisorSearch";

/**
 * A search for one person or institution of the organization. The server searches by name (ADR
 * 0069); the member types and picks. The picker keeps its own text. A parent that wants a fresh
 * picker, for example after a save, gives it a new `key`.
 */
export function PartyPicker({
  api,
  label,
  placeholder,
  error,
  onChange,
}: {
  api: Api;
  label: string;
  placeholder?: string;
  error?: string | undefined;
  /** The chosen party (`person:{uuid}` or `institution:{uuid}`), or nothing when the member clears it. */
  onChange: (party: Choice | undefined) => void;
}) {
  const [text, setText] = useState("");
  const [chosen, setChosen] = useState<Choice>();
  // The search stops while the text shows the label of the chosen option.
  const search = usePromisorSearch(api, text, text !== chosen?.label);
  return (
    <ComboBox
      label={label}
      {...(placeholder !== undefined && { placeholder })}
      options={search.options}
      inputValue={text}
      onInputChange={(next) => {
        setText(next);
        // React Aria keeps a choice while both the text and the key are controlled; a text that
        // the member emptied is no choice any more.
        if (next === "" && chosen !== undefined) {
          setChosen(undefined);
          onChange(undefined);
        }
      }}
      selectedKey={chosen?.id}
      onSelectionChange={(id) => {
        const party = search.options.find((option) => option.id === id);
        setChosen(party);
        onChange(party);
      }}
      isLoading={search.loading}
      error={error ?? search.failure}
    />
  );
}
