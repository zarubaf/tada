import { IconPlus } from "@tabler/icons-react";
import { useRef, useState } from "react";
import type { Api } from "../api/client";
import { t } from "../i18n";
import { useNarrow } from "../shell/useNarrow";
import { Button } from "../ui/Button";
import { Page, PageTitle } from "../ui/Page";
import { PageToolbar } from "../ui/PageToolbar";
import { SearchField } from "../ui/SearchField";
import { TabPanel, Tabs } from "../ui/Tabs";
import styles from "./ContactsPage.module.css";
import { CONTACT_TABS, KIND_OF_TAB, useContactsTab } from "./contactsTab";
import { PartyRegister } from "./PartyRegister";

/**
 * „Kontakte“ (ADR 0082): the persons and the institutions of the organization on one page. The
 * tabs sit in the toolbar row beside the title; the search and „Neu“ at its end belong to the tab
 * that shows. The tab is in the query, as the view of the „Arbeit“ page is.
 */
export function ContactsPage({ api }: { api: Api }) {
  const [tab, setTab] = useContactsTab();
  const kind = KIND_OF_TAB[tab];
  const narrow = useNarrow();
  const [query, setQuery] = useState("");
  const [creating, setCreating] = useState(false);
  const heading = useRef<HTMLHeadingElement>(null);
  const size = narrow ? "md" : "sm";

  const choose = (next: typeof tab) => {
    // A search for a person means nothing in the register of institutions.
    setQuery("");
    setCreating(false);
    setTab(next);
  };

  return (
    <Page width="flush">
      <Tabs
        label={t("contacts-tabs")}
        tabs={CONTACT_TABS.map((id) => ({ id, label: t(`contacts-tab-${id}`) }))}
        value={tab}
        variant="segmented"
        renderList={(list) => (
          <PageToolbar>
            <div className={styles.lead}>
              <PageTitle ref={heading} compact>
                {t("contacts-title")}
              </PageTitle>
              {list}
            </div>
            <div className={styles.end}>
              <div className={styles.search}>
                <SearchField
                  label={t("parties-search")}
                  placeholder={t("parties-search")}
                  value={query}
                  onChange={setQuery}
                />
              </div>
              <Button
                variant="primary"
                size={size}
                aria-label={t(`${kind}-new`)}
                onPress={() => setCreating(true)}
              >
                <IconPlus size={14} stroke={1.5} aria-hidden="true" />
                {t("parties-new")}
              </Button>
            </div>
          </PageToolbar>
        )}
        onChange={(id) => choose(CONTACT_TABS.find((candidate) => candidate === id) ?? "persons")}
      >
        <TabPanel id={tab}>
          <div className={styles.content}>
            <PartyRegister
              key={kind}
              api={api}
              kind={kind}
              query={query}
              label={t(`contacts-tab-${tab}`)}
              heading={heading}
              creating={creating}
              onCloseCreate={() => setCreating(false)}
            />
          </div>
        </TabPanel>
      </Tabs>
    </Page>
  );
}
