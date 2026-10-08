import { type FormEvent, useRef, useState } from "react";
import type { Api } from "../api/client";
import { type Failure, failureOf, useWaiting } from "../api/failure";
import { t } from "../i18n";
import { Button } from "../ui/Button";
import { firstInvalidField, useFocusAfterCommit } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { LiveRegion } from "../ui/LiveRegion";
import { TextField } from "../ui/TextField";
import { formClass, PublicPage, PublicText } from "./PublicPage";

/**
 * „Anmelden“: asks for a magic link. The answer is the same for each address, so that the page
 * does not tell whether an address belongs to a member (ADR 0008).
 */
export function SignInPage({ api }: { api: Api }) {
  const [email, setEmail] = useState("");
  const [busy, setBusy] = useState(false);
  const [sent, setSent] = useState(false);
  const [failure, setFailure] = useState<Failure>();
  const waiting = useWaiting(failure);
  // The page checks the address itself, so that the message comes from Fluent, not the browser.
  const [invalid, setInvalid] = useState(false);
  const form = useRef<HTMLFormElement>(null);
  const focusAfterCommit = useFocusAfterCommit();

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const wrong = !/^[^\s@]+@[^\s@]+$/.test(email.trim());
    setInvalid(wrong);
    if (wrong) {
      // After a failed submit, focus goes to the first invalid field.
      focusAfterCommit(() => firstInvalidField(form.current));
      return;
    }
    if (busy || waiting) {
      return;
    }
    setBusy(true);
    setSent(false);
    setFailure(undefined);
    try {
      const result = await api.POST("/api/v1/sign-in/requests", { body: { email: email.trim() } });
      if (result.error) {
        setFailure(failureOf(result));
      } else {
        setSent(true);
      }
    } catch {
      setFailure(failureOf({}));
    }
    setBusy(false);
  };

  return (
    <PublicPage title={t("sign-in-title")}>
      <PublicText>{t("sign-in-text")}</PublicText>
      <form ref={form} className={formClass} onSubmit={(event) => void submit(event)} noValidate>
        <TextField
          label={t("sign-in-email")}
          type="email"
          name="email"
          autoComplete="email"
          inputMode="email"
          isRequired
          value={email}
          onChange={setEmail}
          error={invalid ? t("sign-in-error-email") : undefined}
        />
        {/* isPending keeps the button focusable, also during a wait; isDisabled would drop focus. */}
        <Button type="submit" variant="primary" isPending={busy || waiting}>
          {t("sign-in-submit")}
        </Button>
      </form>
      <LiveRegion kind="status">{sent ? t("sign-in-sent") : ""}</LiveRegion>
      <LiveRegion kind="alert" visuallyHidden>
        {failure?.message}
      </LiveRegion>
      {failure && (
        <InlineError
          key={failure.id}
          message={failure.message}
          requestId={failure.requestId}
          announce="none"
        />
      )}
    </PublicPage>
  );
}
