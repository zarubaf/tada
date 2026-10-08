import { type FormEvent, useEffect, useRef, useState } from "react";
import {
  type Api,
  type Invitation,
  type OrganizationRole,
  type Problem,
  problemMessage,
} from "../api/client";
import { failureOf } from "../api/failure";
import { uuidv7 } from "../api/uuid";
import { hasMessage, t } from "../i18n";
import { Button } from "../ui/Button";
import { Select } from "../ui/Select";
import { TextField } from "../ui/TextField";
import styles from "./MembersPage.module.css";

type Field = "email" | "display_name";
type FieldErrors = Partial<Record<Field, string>>;

/** The message for each invalid value of a `validation-failed` problem that has one. */
function fieldErrors(problem: Problem): FieldErrors {
  const result: FieldErrors = {};
  for (const { pointer, code } of problem.errors ?? []) {
    const field = pointer.slice(1);
    if (field === "email" || field === "display_name") {
      const specific = `invite-error-${field}-${code}`;
      result[field] = t(hasMessage(specific) ? specific : `invite-error-${field}`);
    }
  }
  return result;
}

/**
 * „Mitglied einladen“: email, name and role. The page owns the live regions, so the form reports
 * the result: `onInvited` for a sent invitation, `onFailed` with the message of any other failure.
 */
export function InviteMemberForm({
  api,
  roles,
  onInvited,
  onFailed,
  onStart,
}: {
  api: Api;
  /** The roles that the caller may give. */
  roles: OrganizationRole[];
  onInvited: (invitation: Invitation) => void;
  onFailed: (message: string) => void;
  onStart: () => void;
}) {
  const [email, setEmail] = useState("");
  const [name, setName] = useState("");
  const [role, setRole] = useState<OrganizationRole>("member");
  // The ID makes a retry safe. A changed value is a different invitation, so it gets a new ID.
  const [id, setId] = useState(() => uuidv7());
  const [errors, setErrors] = useState<FieldErrors>({});
  const [busy, setBusy] = useState(false);
  const form = useRef<HTMLFormElement>(null);
  // Count the submits that move focus: to the first invalid field, or after a success back to
  // the first field.
  const [failedSubmits, setFailedSubmits] = useState(0);
  const [sent, setSent] = useState(0);

  useEffect(() => {
    if (failedSubmits > 0) {
      form.current?.querySelector<HTMLElement>("[aria-invalid='true']")?.focus();
    }
  }, [failedSubmits]);

  useEffect(() => {
    if (sent > 0) {
      form.current?.querySelector<HTMLElement>("input")?.focus();
    }
  }, [sent]);

  const edit = (set: (value: string) => void) => (value: string) => {
    set(value);
    setId(uuidv7());
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const local: FieldErrors = {};
    if (!email.includes("@")) {
      local.email = t("invite-error-email");
    }
    if (name.trim() === "") {
      local.display_name = t("invite-error-display_name");
    }
    setErrors(local);
    onStart();
    if (Object.keys(local).length > 0) {
      setFailedSubmits((count) => count + 1);
      return;
    }
    if (busy) {
      return;
    }

    setBusy(true);
    try {
      const { data, error, response } = await api.POST("/api/v1/invitations", {
        body: { id, email: email.trim(), display_name: name.trim(), role },
      });
      if (data) {
        setEmail("");
        setName("");
        setId(uuidv7());
        onInvited(data);
        setSent((count) => count + 1);
      } else {
        const invalid = error?.code === "validation-failed" ? fieldErrors(error) : {};
        setErrors(invalid);
        if (Object.keys(invalid).length === 0) {
          onFailed(failureOf({ error, response }).message);
        } else {
          setFailedSubmits((count) => count + 1);
        }
      }
    } catch {
      onFailed(problemMessage(undefined));
    }
    setBusy(false);
  };

  return (
    <section className={styles.invite} aria-labelledby="invite-title">
      <h2 id="invite-title" className={styles.heading}>
        {t("invite-title")}
      </h2>
      <form ref={form} className={styles.form} onSubmit={(event) => void submit(event)} noValidate>
        <TextField
          label={t("invite-name")}
          value={name}
          onChange={edit(setName)}
          error={errors.display_name}
          autoComplete="off"
          isRequired
        />
        <TextField
          label={t("invite-email")}
          type="email"
          value={email}
          onChange={edit(setEmail)}
          error={errors.email}
          autoComplete="off"
          isRequired
        />
        <Select
          label={t("invite-role")}
          options={roles.map((r) => ({ id: r, label: t(`role-${r}`) }))}
          value={role}
          onChange={(selected) => {
            setRole(selected as OrganizationRole);
            setId(uuidv7());
          }}
        />
        <div className={styles.actions}>
          <Button type="submit" variant="primary" isPending={busy}>
            {t("invite-submit")}
          </Button>
        </div>
      </form>
    </section>
  );
}
