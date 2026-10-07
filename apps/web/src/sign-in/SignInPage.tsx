import { type FormEvent, useState } from "react";
import { type Api, problemMessage } from "../api/client";
import { t } from "../i18n";
import { Button } from "../ui/Button";
import { InlineError } from "../ui/InlineError";
import { TextField } from "../ui/TextField";
import { formClass, PublicPage, PublicText } from "./PublicPage";

type State =
  | { kind: "idle" }
  | { kind: "sending" }
  | { kind: "sent" }
  | { kind: "failed"; message: string; requestId: string | undefined };

/**
 * „Anmelden“: asks for a magic link. The answer is the same for each address, so that the page
 * does not tell whether an address belongs to a member (ADR 0008).
 */
export function SignInPage({ api }: { api: Api }) {
  const [email, setEmail] = useState("");
  const [state, setState] = useState<State>({ kind: "idle" });

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setState({ kind: "sending" });
    try {
      const { error } = await api.POST("/api/v1/sign-in/requests", { body: { email } });
      setState(
        error
          ? { kind: "failed", message: problemMessage(error), requestId: error.request_id }
          : { kind: "sent" },
      );
    } catch {
      setState({ kind: "failed", message: problemMessage(undefined), requestId: undefined });
    }
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
        <Button type="submit" variant="primary" isDisabled={state.kind === "sending"}>
          {t("sign-in-submit")}
        </Button>
      </form>
      {state.kind === "sent" && <p role="status">{t("sign-in-sent")}</p>}
      {state.kind === "failed" && (
        <InlineError message={state.message} requestId={state.requestId} />
      )}
    </PublicPage>
  );
}
