// The choices of an enumeration control: a closed list of values that each have a word and, if the
// kind gives one, a small icon (the level of a risk). The word names the value; the icon only
// repeats it, so that no value shows by an icon or a color alone.
import { t } from "../../i18n";
import type { Choices } from "./choices";
import type { FieldSpec } from "./descriptor";

/** The choices of an enum field: all its values, for a record and for the form of a new one. */
export function enumChoices(spec: FieldSpec): Choices {
  const values = spec.values ?? [];
  return {
    options: values.map((value) => {
      const { labelKey, icon: Icon } = spec.present?.(value) ?? { labelKey: value };
      return {
        id: value,
        label: t(labelKey),
        ...(Icon && { leading: <Icon size={14} aria-hidden="true" /> }),
      };
    }),
    offered: values.length > 1,
  };
}
