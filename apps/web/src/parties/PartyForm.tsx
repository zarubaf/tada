import { type FormEvent, useRef, useState } from "react";
import { type Problem, problemMessage } from "../api/client";
import { hasMessage, t } from "../i18n";
import { type SaveFailure, saveFailure } from "../registers/saveFailure";
import { Button } from "../ui/Button";
import { firstInvalidField, useFocusAfterCommit } from "../ui/focus";
import { Select } from "../ui/Select";
import { TextField } from "../ui/TextField";
import styles from "./PartiesPage.module.css";
import type { Party, PartyApi, PartyKind } from "./partyApi";

const KINDS = ["authority", "company", "club", "other"] as const;

type Field = "name" | "email" | "phone";
type FieldErrors = Partial<Record<Field, string>>;

/** The message for each invalid value of a `validation-failed` problem that has one. */
function fieldErrors(problem: Problem): FieldErrors {
  const result: FieldErrors = {};
  for (const { pointer, code } of problem.errors ?? []) {
    const field = pointer.slice(1);
    if (field === "name" || field === "email" || field === "phone") {
      const specific = `party-error-${field}-${code}`;
      result[field] = t(hasMessage(specific) ? specific : `party-error-${field}`);
    }
  }
  return result;
}

/** What the member typed; an empty optional field is no value. */
function blankToNull(text: string): string | null {
  return text.trim() === "" ? null : text.trim();
}

/**
 * „Person erfassen“, „Institution erfassen“ and the change of a record. The page owns the live
 * regions, so the form reports the result: `onSaved` for a saved record, `onFailed` for any other
 * failure.
 */
export function PartyForm({
  api,
  kind,
  record,
  onSaved,
  onFailed,
  onStart,
  onCancel,
}: {
  api: PartyApi;
  kind: PartyKind;
  /** The record to change. Without it, the form creates a record. */
  record?: Party;
  onSaved: (record: Party) => void;
  onFailed: (failure: SaveFailure) => void;
  onStart: () => void;
  onCancel?: () => void;
}) {
  const [name, setName] = useState(record?.name ?? "");
  const [email, setEmail] = useState(record?.email ?? "");
  const [phone, setPhone] = useState(record?.phone ?? "");
  const [partyKind, setPartyKind] = useState(record && "kind" in record ? record.kind : "other");
  const [errors, setErrors] = useState<FieldErrors>({});
  const [busy, setBusy] = useState(false);
  const form = useRef<HTMLFormElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  const focusInvalidField = () => focusAfterCommit(() => firstInvalidField(form.current));
  const changing = record !== undefined;

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    onStart();
    if (busy) {
      return;
    }
    if (name.trim() === "") {
      setErrors({ name: t("party-error-name") });
      focusInvalidField();
      return;
    }
    setErrors({});
    setBusy(true);
    try {
      const input = {
        name: name.trim(),
        email: blankToNull(email),
        phone: blankToNull(phone),
        ...(kind === "institution" && { kind: partyKind }),
      };
      const { data, error, response } = changing
        ? await api.change(record, input)
        : await api.create(input);
      if (data) {
        if (!changing) {
          setName("");
          setEmail("");
          setPhone("");
          setPartyKind("other");
          focusAfterCommit(() => form.current?.querySelector<HTMLElement>("input"));
        }
        onSaved(data);
      } else {
        const invalid = error?.code === "validation-failed" ? fieldErrors(error) : {};
        setErrors(invalid);
        if (Object.keys(invalid).length > 0) {
          focusInvalidField();
        } else {
          onFailed(saveFailure({ error, response }));
        }
      }
    } catch {
      onFailed({ message: problemMessage(undefined), conflict: false });
    }
    setBusy(false);
  };

  return (
    <form ref={form} className={styles.form} onSubmit={(event) => void submit(event)} noValidate>
      <TextField
        label={t("party-name")}
        value={name}
        onChange={setName}
        error={errors.name}
        autoComplete="off"
        isRequired
      />
      {kind === "institution" && (
        <Select
          label={t("party-kind")}
          options={KINDS.map((k) => ({ id: k, label: t(`institution-kind-${k}`) }))}
          value={partyKind}
          onChange={setPartyKind}
        />
      )}
      <TextField
        label={t("party-email")}
        type="email"
        value={email}
        onChange={setEmail}
        error={errors.email}
        autoComplete="off"
      />
      <TextField
        label={t("party-phone")}
        type="tel"
        value={phone}
        onChange={setPhone}
        error={errors.phone}
        autoComplete="off"
      />
      <div className={styles.actions}>
        {onCancel && (
          <Button onPress={onCancel} isDisabled={busy}>
            {t("party-cancel")}
          </Button>
        )}
        <Button type="submit" variant="primary" isPending={busy}>
          {changing ? t("party-save") : t("party-create")}
        </Button>
      </div>
    </form>
  );
}
