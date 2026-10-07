import { type FormEvent, useState } from "react";
import type { Api } from "../api/client";
import { type Failure, failureOf, useWaiting } from "../api/failure";
import { t } from "../i18n";
import { Button } from "../ui/Button";
import { InlineError } from "../ui/InlineError";
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

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (busy || waiting) {
      return;
    }
    setBusy(true);
    setSent(false);
    setFailure(undefined);
    try {
      const result = await api.POST("/api/v1/sign-in/requests", { body: { email } });
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
      <form className={formClass} onSubmit={(event) => void submit(event)}>
        <TextField
          label={t("sign-in-email")}
          type="email"
          name="email"
          autoComplete="email"
          inputMode="email"
          isRequired
          value={email}
          onChange={setEmail}
        />
        {/* isPending keeps the button focusable; isDisabled would drop the focus. */}
        <Button type="submit" variant="primary" isPending={busy} isDisabled={waiting}>
          {t("sign-in-submit")}
        </Button>
      </form>
      {/* The live region exists before its text, so that screen readers announce the change. */}
      <p role="status">{sent ? t("sign-in-sent") : ""}</p>
      {failure && (
        <InlineError
          key={failure.id}
          message={failure.message}
          requestId={failure.requestId}
          takeFocus
        />
      )}
    </PublicPage>
  );
}
