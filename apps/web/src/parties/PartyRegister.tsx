import { lazy, type RefObject, Suspense, useCallback, useMemo } from "react";
import type { Api } from "../api/client";
import { t } from "../i18n";
import { canManage } from "../members/roles";
import { RegisterView } from "../registers/RegisterView";
import { useRegister } from "../registers/useRegister";
import { useRegisterPage } from "../registers/useRegisterPage";
import { useSession } from "../session/SessionProvider";
import { Button } from "../ui/Button";
import type { Column } from "../ui/DataTable";
import { LiveRegion } from "../ui/LiveRegion";
import { redactedOr } from "../ui/Redacted";
import { Skeleton } from "../ui/Skeleton";
import { TaskDialog } from "../ui/TaskDialog";
import { PartyForm } from "./PartyForm";
import styles from "./PartyRegister.module.css";
import { type Party, type PartyKind, partyApi } from "./partyApi";

// Only an owner opens it, so the initial bundle does not carry it (ADR 0024).
const PersonDataProtection = lazy(() => import("./PersonDataProtection"));

export interface PartyRegisterProps {
  api: Api;
  kind: PartyKind;
  /** The text of the search by name; the page owns the field in its toolbar row. */
  query: string;
  /** The name of the table. */
  label: string;
  /** The `h1` of the page: focus returns to it when a form closes. */
  heading: RefObject<HTMLHeadingElement | null>;
  /** True while the form of a new record is open in its dialog. */
  creating: boolean;
  onCloseCreate: () => void;
}

/**
 * One register of „Kontakte“, persons or institutions: the table, the form that changes a record
 * and the dialog that creates one. The page shows „Bearbeiten“ only where the record says
 * `can_change`.
 */
export function PartyRegister({
  api,
  kind,
  query,
  label,
  heading,
  creating,
  onCloseCreate,
}: PartyRegisterProps) {
  const party = useMemo(() => partyApi(api, kind), [api, kind]);
  const person = kind === "person";
  const role = useSession().organization?.role;
  const manages = canManage(role);
  const search = query.trim();
  // A new search is a new `fetchPage`, so the register loads the first page again.
  const fetchPage = useCallback(
    (cursor: string | undefined) =>
      party.list({ ...(search === "" ? {} : { q: search }), ...(cursor && { cursor }) }),
    [party, search],
  );
  const register = useRegister<Party>(fetchPage);
  const page = useRegisterPage<Party>(register.reload, heading);
  const { editing } = page;

  const saved = (record: Party) => {
    page.setConfirmation(page.savedMessage(record.name));
    if (editing) {
      register.replace(record);
      page.closeForm();
    } else {
      onCloseCreate();
      if (search === "") {
        // A new record has the highest number, so it belongs at the end of the loaded rows. With
        // a search, it may not match, so the list loads again.
        register.append(record);
      } else {
        void register.reload();
      }
    }
  };

  const columns: Column<Party>[] = [
    { id: "id", header: t("parties-column-id"), cell: (row) => row.local_id, mono: true },
    {
      id: "name",
      header: t("parties-column-name"),
      cell: (row) => redactedOr(row.redacted, row.name),
    },
    ...(person
      ? []
      : [
          {
            id: "kind",
            header: t("parties-column-kind"),
            cell: (row: Party) => ("kind" in row ? t(`institution-kind-${row.kind}`) : ""),
          },
        ]),
    { id: "email", header: t("parties-column-email"), cell: (row) => row.email ?? "" },
    { id: "phone", header: t("parties-column-phone"), cell: (row) => row.phone ?? "" },
    ...(register.state.kind === "loaded" &&
    register.state.items.some((row) => row.can_change && !row.redacted)
      ? [
          {
            id: "actions",
            header: t("parties-column-actions"),
            cell: (row: Party) =>
              row.can_change &&
              !row.redacted && (
                <Button
                  size="sm"
                  aria-label={t("parties-edit-of", { name: row.name })}
                  onPress={() => page.edit(row)}
                >
                  {t("parties-edit")}
                </Button>
              ),
          },
        ]
      : []),
  ];

  return (
    <>
      <LiveRegion kind="alert">{page.failure}</LiveRegion>
      <LiveRegion kind="status">{page.confirmation}</LiveRegion>

      <RegisterView
        register={register}
        page={page}
        label={label}
        columns={columns}
        rowId={(row) => row.local_id}
        loadingLabel={t("parties-loading")}
        empty={
          search === ""
            ? { title: t("parties-empty-title"), text: t("parties-empty-text") }
            : { title: t("parties-no-match-title"), text: t("parties-no-match-text") }
        }
      />

      {creating && (
        <TaskDialog
          title={person ? t("person-create-title") : t("institution-create-title")}
          onCancel={onCloseCreate}
        >
          <PartyForm
            api={party}
            kind={kind}
            {...(person && manages && { memberApi: api })}
            onStart={page.start}
            onSaved={saved}
            onFailed={page.fail}
            onCancel={onCloseCreate}
          />
        </TaskDialog>
      )}

      {editing && (
        <section className={styles.section} aria-labelledby="party-form-title">
          <h2 id="party-form-title" ref={page.formHeading} tabIndex={-1} className={styles.heading}>
            {t("party-change-title", { name: editing.name })}
          </h2>
          <PartyForm
            key={`${editing.id}:${editing.version}`}
            api={party}
            kind={kind}
            {...(person && manages && { memberApi: api })}
            record={editing}
            onStart={page.start}
            onSaved={saved}
            onFailed={page.fail}
            onCancel={() => page.closeForm()}
          />
        </section>
      )}

      {person && role === "owner" && editing && (
        <Suspense fallback={<Skeleton />}>
          <PersonDataProtection key={editing.id} api={api} personId={editing.id} />
        </Suspense>
      )}
    </>
  );
}
