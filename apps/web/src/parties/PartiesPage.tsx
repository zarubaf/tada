import { useCallback, useMemo, useState } from "react";
import type { Api } from "../api/client";
import { t } from "../i18n";
import { RegisterView } from "../registers/RegisterView";
import { useRegister } from "../registers/useRegister";
import { useRegisterPage } from "../registers/useRegisterPage";
import { Button } from "../ui/Button";
import type { Column } from "../ui/DataTable";
import { LiveRegion } from "../ui/LiveRegion";
import { Page, PageTitle } from "../ui/Page";
import { TextField } from "../ui/TextField";
import styles from "./PartiesPage.module.css";
import { PartyForm } from "./PartyForm";
import { type Party, type PartyKind, partyApi } from "./partyApi";
import { useAccounts } from "./useAccounts";

/**
 * „Personen“ and „Institutionen“: the register of the organization, a search by name and the form
 * that creates a record. The page shows „Bearbeiten“ only where the record says `can_change`.
 */
export function PartiesPage({ api, kind }: { api: Api; kind: PartyKind }) {
  const party = useMemo(() => partyApi(api, kind), [api, kind]);
  const person = kind === "person";
  const accounts = useAccounts(api);
  const [query, setQuery] = useState("");
  const search = query.trim();
  // A new search is a new `fetchPage`, so the register loads the first page again.
  const fetchPage = useCallback(
    (cursor: string | undefined) =>
      party.list({ ...(search === "" ? {} : { q: search }), ...(cursor && { cursor }) }),
    [party, search],
  );
  const register = useRegister<Party>(fetchPage);
  const page = useRegisterPage<Party>(register.reload);
  const { editing } = page;

  const saved = (record: Party) => {
    page.setConfirmation(page.savedMessage(record.name));
    if (editing) {
      register.replace(record);
      page.closeForm();
    } else if (search === "") {
      // A new record has the highest number, so it belongs at the end of the loaded rows. With a
      // search, it may not match, so the list loads again.
      register.append(record);
    } else {
      void register.reload();
    }
  };

  const columns: Column<Party>[] = [
    { id: "id", header: t("parties-column-id"), cell: (row) => row.local_id, mono: true },
    { id: "name", header: t("parties-column-name"), cell: (row) => row.name },
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
    ...(register.state.kind === "loaded" && register.state.items.some((row) => row.can_change)
      ? [
          {
            id: "actions",
            header: t("parties-column-actions"),
            cell: (row: Party) =>
              row.can_change && (
                <Button
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

  const title = person ? t("persons-title") : t("institutions-title");

  return (
    <Page>
      <LiveRegion kind="alert">{page.failure}</LiveRegion>
      <LiveRegion kind="status">{page.confirmation}</LiveRegion>
      <div className={styles.toolbar}>
        <PageTitle ref={page.heading}>{title}</PageTitle>
        <div className={styles.search}>
          <TextField
            label={t("parties-search")}
            type="search"
            value={query}
            onChange={setQuery}
            autoComplete="off"
          />
        </div>
      </div>

      <RegisterView
        register={register}
        page={page}
        label={title}
        columns={columns}
        loadingLabel={t("parties-loading")}
        empty={
          search === ""
            ? { title: t("parties-empty-title"), text: t("parties-empty-text") }
            : { title: t("parties-no-match-title"), text: t("parties-no-match-text") }
        }
      />

      <section className={styles.section} aria-labelledby="party-form-title">
        <h2 id="party-form-title" ref={page.formHeading} tabIndex={-1} className={styles.heading}>
          {editing
            ? t("party-change-title", { name: editing.name })
            : person
              ? t("person-create-title")
              : t("institution-create-title")}
        </h2>
        <PartyForm
          key={editing ? `${editing.id}:${editing.version}` : "new"}
          api={party}
          kind={kind}
          {...(person && accounts && { accounts })}
          {...(editing && { record: editing })}
          onStart={page.start}
          onSaved={saved}
          onFailed={page.fail}
          {...(editing && { onCancel: () => page.closeForm() })}
        />
      </section>
    </Page>
  );
}
