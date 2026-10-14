// The field `people` of the create form and of the dialogs: a picker for several persons and
// institutions, searched on the server (ADR 0069). The value is the IDs of the chosen people
// (`peopleValue`), so the form keeps one kind of value; the field remembers their names.
import { useState } from "react";
import type { Api } from "../../api/client";
import { usePromisorSearch } from "../usePromisorSearch";
import { type FieldValue, peopleIds, peopleValue } from "./descriptor";
import { PeoplePicker, type Person } from "./PeoplePicker";

export function PeopleField({
  label,
  api,
  value,
  onChange,
  seed = [],
  error,
}: {
  label: string;
  api: Api;
  /** The IDs of the chosen people. */
  value: FieldValue;
  onChange: (value: FieldValue) => void;
  /** The names of people that the value holds from the start, for example the parties of the decision that a successor replaces. */
  seed?: Person[];
  error?: string | undefined;
}) {
  const [text, setText] = useState("");
  const search = usePromisorSearch(api, text, true);
  // A chosen ID has the name that the search showed when the member chose it.
  const [known] = useState(() => new Map(seed.map((person) => [person.id, person])));
  const people = peopleIds(value).map((id) => known.get(id) ?? { id, label: id });
  return (
    <PeoplePicker
      label={label}
      options={search.options}
      value={people}
      onChange={(next) => {
        for (const person of next) {
          known.set(person.id, person);
        }
        onChange(peopleValue(next.map((person) => person.id)));
      }}
      search={text}
      onSearch={setText}
      loading={search.loading}
      failure={search.failure}
      error={error}
    />
  );
}
