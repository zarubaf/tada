import { useState } from "react";
import { t } from "../i18n";
import { Button } from "../ui/Button";
import { useSession } from "./SessionProvider";

/**
 * The way out of `recent-sign-in-required` (ADR 0037): the button ends the session and opens the
 * sign-in page, so that a magic link starts a new session. The page shows the message itself.
 */
export function SignInAgain() {
  const session = useSession();
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string>();
  const signOut = async () => {
    setBusy(true);
    setFailure(await session.signOut());
    setBusy(false);
  };
  return (
    <div>
      <Button variant="primary" isPending={busy} onPress={() => void signOut()}>
        {t("sign-in-again")}
      </Button>
      {failure && <p role="alert">{failure}</p>}
    </div>
  );
}
