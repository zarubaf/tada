// The controls that only the create form of a record has: a field that the panel never changes
// afterwards, so that no editor in `FieldEditor` knows it (the promisor of a commitment is chosen
// once). A control joins by one row here and by its `ControlKind`; the form takes care of the label,
// the required mark and the error, and the control draws the input.
import type { ComponentType } from "react";
import type { Api } from "../../api/client";
import type { ControlKind, FieldSpec, FieldValue } from "./descriptor";
import { PartyControl } from "./PartyControl";

export interface CreateControlProps {
  spec: FieldSpec;
  /** The label with the mark of a required field. */
  label: string;
  value: FieldValue;
  error: string | undefined;
  onChange: (value: FieldValue) => void;
  api: Api;
}

export interface CreateControl {
  /** True if the control has its own full-width row above the property rows, as a text field has. */
  stacked: boolean;
  Control: ComponentType<CreateControlProps>;
}

export const CREATE_CONTROLS: Partial<Record<ControlKind, CreateControl>> = {
  party: { stacked: true, Control: PartyControl },
};
