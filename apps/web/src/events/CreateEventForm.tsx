import { type FormEvent, useEffect, useRef, useState } from "react";
import { type Api, type Problem, problemMessage } from "../api/client";
import { uuidv7 } from "../api/uuid";
import { hasMessage, t } from "../i18n";
import { useNavigate } from "../router/Router";
import { Button } from "../ui/Button";
import { TextField } from "../ui/TextField";
import styles from "./Form.module.css";

const DEFAULT_TIME_ZONE = "Europe/Zurich";
/** The rule of the server: two to eight capital letters and digits. */
const KEY_PATTERN = /^[A-Z0-9]{2,8}$/;

type Field = "key" | "name" | "time_zone";
type FieldErrors = Partial<Record<Field, string>>;

/** The message for each invalid value of a `validation-failed` problem that has one. */
function fieldErrors(problem: Problem): FieldErrors {
  const result: FieldErrors = {};
  for (const { pointer, code } of problem.errors ?? []) {
    const field = pointer.slice(1);
    if (field === "key" || field === "name" || field === "time_zone") {
      const specific = `event-error-${field}-${code}`;
      result[field] = t(hasMessage(specific) ? specific : `event-error-${field}`);
    }
  }
  return result;
}

/** „Anlass erfassen“: key, name and time zone of a new event. */
export function CreateEventForm({ api }: { api: Api }) {
  const navigate = useNavigate();
  const [key, setKey] = useState("");
  const [name, setName] = useState("");
  const [timeZone, setTimeZone] = useState(DEFAULT_TIME_ZONE);
  // The ID makes a retry safe. A changed value is a different event, so it gets a new ID.
  const [id, setId] = useState(() => uuidv7());
  const [errors, setErrors] = useState<FieldErrors>({});
  const [failure, setFailure] = useState<string>();
  const [busy, setBusy] = useState(false);
  const form = useRef<HTMLFormElement>(null);
  // Counts the failed submits: after each one, focus goes to the first invalid field.
  const [failedSubmits, setFailedSubmits] = useState(0);

  useEffect(() => {
    if (failedSubmits > 0) {
      form.current?.querySelector<HTMLElement>("[aria-invalid='true']")?.focus();
    }
  }, [failedSubmits]);

  const edit = (set: (value: string) => void) => (value: string) => {
    set(value);
    setId(uuidv7());
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const normalizedKey = key.trim().toUpperCase();
    const local: FieldErrors = {};
    if (!KEY_PATTERN.test(normalizedKey)) {
      local.key = t("event-error-key");
    }
    if (name.trim() === "") {
      local.name = t("event-error-name");
    }
    setErrors(local);
    setFailure(undefined);
    if (Object.keys(local).length > 0) {
      setFailedSubmits((count) => count + 1);
      return;
    }
    if (busy) {
      return;
    }

    setBusy(true);
    try {
      const { data, error } = await api.POST("/api/v1/events", {
        body: { id, key: normalizedKey, name: name.trim(), time_zone: timeZone.trim() },
      });
      if (data) {
        navigate(`/events/${data.id}`);
        return;
      }
      const invalid = error?.code === "validation-failed" ? fieldErrors(error) : {};
      setErrors(invalid);
      if (Object.keys(invalid).length === 0) {
        setFailure(problemMessage(error));
      } else {
        setFailedSubmits((count) => count + 1);
      }
    } catch {
      setFailure(problemMessage(undefined));
    }
    setBusy(false);
  };

  return (
    <main id="main" className={styles.page}>
      <h1 className={styles.title}>{t("event-create-title")}</h1>
      <form ref={form} className={styles.form} onSubmit={(event) => void submit(event)} noValidate>
        <TextField
          label={t("events-column-key")}
          help={t("event-create-key-help")}
          value={key}
          onChange={edit(setKey)}
          error={errors.key}
          autoComplete="off"
          mono
          isRequired
        />
        <TextField
          label={t("events-column-name")}
          value={name}
          onChange={edit(setName)}
          error={errors.name}
          autoComplete="off"
          isRequired
        />
        <TextField
          label={t("events-column-time-zone")}
          help={t("event-create-time-zone-help")}
          value={timeZone}
          onChange={edit(setTimeZone)}
          error={errors.time_zone}
          autoComplete="off"
          isRequired
        />
        <p className={styles.failure} role="alert">
          {failure}
        </p>
        <div className={styles.actions}>
          <Button type="submit" variant="primary" isPending={busy}>
            {t("event-create-submit")}
          </Button>
        </div>
      </form>
    </main>
  );
}
