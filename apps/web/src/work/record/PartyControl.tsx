import { t } from "../../i18n";
import { PartyPicker } from "../PartiesSelect";
import type { CreateControlProps } from "./createControls";

/**
 * The search for a person or an institution in the create form. The value is the party as
 * `person:{uuid}` or `institution:{uuid}`, or nothing while no party is chosen. The picker keeps the
 * text and the choice itself, so `value` is only the start.
 */
export function PartyControl({ spec, label, error, onChange, api }: CreateControlProps) {
  return (
    <PartyPicker
      api={api}
      label={label}
      error={error}
      {...(spec.promptKey && { placeholder: t(spec.promptKey) })}
      onChange={(party) => onChange(party?.id ?? null)}
    />
  );
}
