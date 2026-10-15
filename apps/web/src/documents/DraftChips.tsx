import {
  IconArchive,
  IconCircleCheck,
  IconEye,
  IconFileText,
  IconHistory,
  IconPencil,
} from "@tabler/icons-react";
import type { DocumentVersion } from "../api/client";
import { t } from "../i18n";
import { Chip } from "../ui/Chip";

const STATUS_ICONS = {
  draft: IconPencil,
  review: IconEye,
  approved: IconCircleCheck,
  superseded: IconHistory,
  archived: IconArchive,
} as const;

/**
 * What a draft version is, as chips with an icon and a word each (ADR 0082): „Entwurf“ and its
 * status. The words always show; the icons only support them.
 */
export function DraftChips({ version }: { version: DocumentVersion }) {
  const status = version.status ?? "draft";
  return (
    <>
      <Chip icon={IconFileText}>{t("document-draft")}</Chip>
      <Chip icon={STATUS_ICONS[status]}>{t(`document-status-${status}`)}</Chip>
    </>
  );
}
