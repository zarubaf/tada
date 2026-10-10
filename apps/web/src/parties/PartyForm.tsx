import { type FormEvent, useRef, useState } from "react";
import { type Api, type Problem, problemMessage } from "../api/client";
import { hasMessage, t } from "../i18n";
import { type SaveFailure, saveFailure } from "../registers/saveFailure";
import { Button } from "../ui/Button";
import { ComboBox } from "../ui/ComboBox";
import { firstInvalidField, useFocusAfterCommit } from "../ui/focus";
import { Select } from "../ui/Select";
import { TextField } from "../ui/TextField";
import styles from "./PartiesPage.module.css";
import type { Party, PartyApi, PartyKind } from "./partyApi";
import { useAccountSearch } from "./useAccountSearch";

const KINDS = ["authority", "company", "club", "other"] as const;

const FIELDS = ["name", "email", "phone", "user_id"] as const;
type Field = (typeof FIELDS)[number];
type FieldErrors = Partial<Record<Field, string>>;

/** The message for each invalid value of a `validation-failed` problem that has one. */
function fieldErrors(problem: Problem): FieldErrors {
  const result: FieldErrors = {};
  for (const { pointer, code } of problem.errors ?? []) {
    const field = pointer.slice(1);
    if (isField(field)) {
      const specific = `party-error-${field}-${code}`;
      result[field] = t(hasMessage(specific) ? specific : `party-error-${field}`);
    }
  }
  return result;
}

function isField(field: string): field is Field {
  return (FIELDS as readonly string[]).includes(field);
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
  memberApi,
  onSaved,
  onFailed,
  onStart,
  onCancel,
}: {
  api: PartyApi;
  kind: PartyKind;
  /** The record to change. Without it, the form creates a record. */
  record?: Party;
  /**
   * The API to search the members that a person can link to. Without it, the form has no account
   * field: only an owner or an admin links a person to an account (ADR 0069).
   */
  memberApi?: Api;
  onSaved: (record: Party) => void;
  onFailed: (failure: SaveFailure) => void;
  onStart: () => void;
  onCancel?: () => void;
}) {
  const [name, setName] = useState(record?.name ?? "");
  const [email, setEmail] = useState(record?.email ?? "");
  const [phone, setPhone] = useState(record?.phone ?? "");
  // The account of a record is an ID without a name here, so the input says only that a link exists.
  const linked = record && "user_id" in record ? (record.user_id ?? undefined) : undefined;
  const linkedText = linked === undefined ? "" : t("party-account-linked");
  const [userId, setUserId] = useState<string | undefined>(linked);
  const [accountText, setAccountText] = useState(linkedText);
  // The text of the chosen option: the search stops while the input shows it.
  const [accountLabel, setAccountLabel] = useState<string | undefined>(linkedText);
  // The same label for the handlers: the combo box reports the choice and its text in one event.
  const chosenLabel = useRef<string | undefined>(linkedText);
  const chooseAccount = (id: string | undefined, label: string | undefined) => {
    setUserId(id);
    setAccountLabel(label);
    chosenLabel.current = label;
  };
  const [partyKind, setPartyKind] = useState(record && "kind" in record ? record.kind : "other");
  const [errors, setErrors] = useState<FieldErrors>({});
  const [busy, setBusy] = useState(false);
  const form = useRef<HTMLFormElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  const focusInvalidField = () => focusAfterCommit(() => firstInvalidField(form.current));
  const changing = record !== undefined;
  const accountSearch = useAccountSearch(memberApi, accountText, accountText !== accountLabel);

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
        ...(memberApi && { user_id: userId ?? null }),
      };
      const { data, error, response } = changing
        ? await api.change(record, input)
        : await api.create(input);
      if (data) {
        if (!changing) {
          setName("");
          setEmail("");
          setPhone("");
          chooseAccount(undefined, undefined);
          setAccountText("");
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
      {kind === "person" && memberApi && (
        <ComboBox
          label={t("party-account")}
          options={accountSearch.options}
          inputValue={accountText}
          onInputChange={(text) => {
            setAccountText(text);
            // Text that no option gave is no choice: the earlier member stays unlinked.
            if (text !== chosenLabel.current) {
              chooseAccount(undefined, chosenLabel.current);
            }
          }}
          selectedKey={userId}
          onSelectionChange={(id) => {
            chooseAccount(id, accountSearch.options.find((option) => option.id === id)?.label);
          }}
          isLoading={accountSearch.loading}
          error={errors.user_id ?? accountSearch.failure}
        />
      )}
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
