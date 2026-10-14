// The fields that more than one kind has with the same words, icon and control: the owner and the
// area. A kind gives the two things that differ, how it reads the value and how it saves it.
import { IconStack2, IconUser } from "@tabler/icons-react";
import type { FieldSpec } from "./descriptor";

type Read = FieldSpec["read"];
type Save = NonNullable<FieldSpec["save"]>;

/** The owner: a member of the event who can own a record. A record always has one. Without `save` it only names choices. */
export function ownerField(read: Read, save?: Save): FieldSpec {
  return {
    id: "owner",
    labelKey: "record-field-owner",
    icon: IconUser,
    control: "person",
    pointer: "owner",
    promptKey: "record-choose-owner",
    required: true,
    read,
    ...(save && { save }),
  };
}

/** The area (workstream): optional, so the picker can clear it. */
export function areaField(read: Read, save?: Save): FieldSpec {
  return {
    id: "area",
    labelKey: "record-field-area",
    icon: IconStack2,
    control: "area",
    pointer: "workstream",
    emptyKey: "record-no-area",
    read,
    ...(save && { save }),
  };
}
