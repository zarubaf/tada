import { IconPlus } from "@tabler/icons-react";
import type { Ref } from "react";
import type { RecordKind } from "../api/client";
import { t } from "../i18n";
import { Button } from "../ui/Button";
import { Menu } from "../ui/Menu";
import { TypeIcon } from "../ui/TypeIcon";

export interface NewMenuProps {
  /** The kinds that the member may create, in the order of the menu. */
  kinds: readonly RecordKind[];
  /** The phone row has the 44 px button; the wide row the compact one. */
  narrow: boolean;
  onCreate: (kind: RecordKind) => void;
  /** The button, for the page that returns focus to it when the create form closes. */
  ref?: Ref<HTMLButtonElement>;
}

/**
 * The „Neu“ button of the „Arbeit“ page. It is a menu of the kinds that the member may create; with
 * one kind it creates that kind at once, and with none there is no button. The primary button
 * of the view, sized like the Filter button beside it.
 */
export function NewMenu({ kinds, narrow, onCreate, ref }: NewMenuProps) {
  const size = narrow ? "md" : "sm";
  const [only] = kinds;
  if (only === undefined) {
    return null;
  }
  if (kinds.length === 1) {
    return (
      <Button ref={ref} variant="primary" size={size} onPress={() => onCreate(only)}>
        <IconPlus size={14} stroke={1.5} aria-hidden="true" />
        {t("work-new")}
      </Button>
    );
  }
  return (
    <Menu
      ref={ref}
      trigger={t("work-new")}
      label={t("work-new")}
      variant="primary"
      size={size}
      triggerIcon={IconPlus}
      placement="bottom end"
      items={kinds.map((kind) => ({
        id: kind,
        label: t(`record-kind-${kind}`),
        icon: <TypeIcon kind={kind} size={16} decorative />,
      }))}
      onAction={(id) => onCreate(id as RecordKind)}
    />
  );
}
