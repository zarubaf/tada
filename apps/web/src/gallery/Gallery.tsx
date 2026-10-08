// The component gallery (ADR 0024): each token and each component in each state.
// Only builds that are not production builds contain this page.
import { Button } from "../ui/Button";
import { Checkbox } from "../ui/Checkbox";
import { type Column, DataTable } from "../ui/DataTable";
import { EmptyState } from "../ui/EmptyState";
import { FileButton } from "../ui/FileButton";
import { FileLink } from "../ui/FileLink";
import { InlineError } from "../ui/InlineError";
import { KnowledgeState } from "../ui/KnowledgeState";
import { Skeleton } from "../ui/Skeleton";
import { Switch } from "../ui/Switch";
import styles from "./Gallery.module.css";

const colors = [
  "bg-canvas",
  "bg-surface",
  "bg-raised",
  "bg-sunken",
  "bg-hover",
  "border-subtle",
  "border-control",
  "text",
  "text-muted",
  "accent",
  "accent-hover",
  "accent-text",
  "accent-subtle",
  "focus",
  "success",
  "success-subtle",
  "warning",
  "warning-subtle",
  "danger",
  "danger-subtle",
];
const fontSizes = ["xs", "sm", "md", "base", "lg", "xl", "2xl", "3xl"];
const spaces = ["0-5", "1", "1-5", "2", "3", "4", "5", "6", "8", "10", "12", "16"];
const radii = ["sm", "md", "lg", "xl"];

// Invented fixtures with long German words (doc/design/principles.md).
interface Row {
  key: string;
  name: string;
  created: string;
}
const rows: Row[] = [
  { key: "TEST30", name: "Tag der offenen Tür Testwil", created: "18.05.2030, 08:00" },
  {
    key: "FLY28",
    name: "Veranstaltungsbewilligungsverfahren Musterhausen",
    created: "01.03.2028, 14:12",
  },
];
const columns: Column<Row>[] = [
  { id: "key", header: "Kürzel", cell: (row) => row.key, mono: true },
  { id: "name", header: "Name", cell: (row) => row.name },
  { id: "created", header: "Erfasst am", cell: (row) => row.created, numeric: true },
];

export function Gallery() {
  return (
    <main className={styles.gallery}>
      <h1 className={styles.title}>Komponenten</h1>

      <section className={styles.section}>
        <h2 className={styles.heading}>Farben</h2>
        <div className={styles.swatches}>
          {colors.map((name) => (
            <div key={name} className={styles.swatch}>
              <span className={styles.chip} style={{ background: `var(--color-${name})` }} />
              <code>--color-{name}</code>
            </div>
          ))}
        </div>
      </section>

      <section className={styles.section}>
        <h2 className={styles.heading}>Schrift</h2>
        {fontSizes.map((size) => (
          <p key={size} style={{ fontSize: `var(--font-size-${size})` }}>
            --font-size-{size}: Veranstaltungsbewilligung 0123456789
          </p>
        ))}
      </section>

      <section className={styles.section}>
        <h2 className={styles.heading}>Abstände und Radien</h2>
        {spaces.map((space) => (
          <div key={space} className={styles.row}>
            <span className={styles.bar} style={{ width: `var(--space-${space})` }} />
            <code>--space-{space}</code>
          </div>
        ))}
        <div className={styles.row}>
          {radii.map((radius) => (
            <span
              key={radius}
              className={styles.radius}
              style={{ borderRadius: `var(--radius-${radius})` }}
            >
              {radius}
            </span>
          ))}
        </div>
      </section>

      <section className={styles.section}>
        <h2 className={styles.heading}>Schaltflächen</h2>
        <div className={styles.row}>
          <Button variant="primary">Anlass erfassen</Button>
          <Button>Erneut versuchen</Button>
          <Button variant="primary" isDisabled>
            Anlass erfassen
          </Button>
          <Button isDisabled>Erneut versuchen</Button>
        </div>
      </section>

      <section className={styles.section}>
        <h2 className={styles.heading}>Auswahl und Schalter</h2>
        <div className={styles.row}>
          <Checkbox label="Hinweis gelesen" isSelected={false} onChange={() => {}} />
          <Checkbox label="Hinweis gelesen" isSelected onChange={() => {}} />
          <Checkbox label="Hinweis gelesen" isSelected isDisabled onChange={() => {}} />
        </div>
        <div className={styles.row}>
          <Switch label="MCP-Token erlauben" isSelected={false} onChange={() => {}} />
          <Switch label="MCP-Token erlauben" isSelected onChange={() => {}} />
          <Switch label="MCP-Token erlauben" isSelected isDisabled onChange={() => {}} />
          <Switch label="MCP-Token erlauben" isSelected isPending onChange={() => {}} />
        </div>
      </section>

      <section className={styles.section}>
        <h2 className={styles.heading}>Dateien</h2>
        <div className={styles.row}>
          <FileButton onSelect={() => {}}>Datei hochladen</FileButton>
          <FileButton isPending onSelect={() => {}}>
            Datei hochladen
          </FileButton>
          <FileLink download href="/beispiel.pdf">
            Herunterladen
          </FileLink>
          <FileLink newTab href="/beispiel.pdf">
            Vorschau in neuem Tab öffnen
          </FileLink>
        </div>
      </section>

      <section className={styles.section}>
        <h2 className={styles.heading}>Zustände</h2>
        <Skeleton />
        <EmptyState
          title="Noch keine Anlässe erfasst"
          text="Hier erscheinen die Anlässe Ihrer Organisation."
        />
        <InlineError
          message="Der Dienst ist im Moment nicht erreichbar."
          requestId="01a1118e-3359-73dd-a500-feed65806a9d"
          onRetry={() => {}}
        />
      </section>

      <section className={styles.section}>
        <h2 className={styles.heading}>Stand des Wissens</h2>
        <div className={styles.row}>
          <KnowledgeState state="accepted" showLabel>
            Flugplatz Musterhausen
          </KnowledgeState>
          <KnowledgeState state="accepted">Flugplatz Musterhausen</KnowledgeState>
          <KnowledgeState state="proposed">CHF 15.00</KnowledgeState>
          <KnowledgeState state="assumption">ca. 20’000 Personen pro Tag</KnowledgeState>
          <KnowledgeState state="conflict">CHF 12.00</KnowledgeState>
          <KnowledgeState state="unknown" />
        </div>
      </section>

      <section className={styles.section}>
        <h2 className={styles.heading}>Tabelle</h2>
        <DataTable
          label="Beispieltabelle"
          columns={columns}
          rows={rows}
          rowKey={(row) => row.key}
        />
      </section>
    </main>
  );
}
