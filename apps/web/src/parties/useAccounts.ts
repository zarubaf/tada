import { useEffect, useState } from "react";
import type { Api } from "../api/client";
import { canManage } from "../members/roles";
import { useSession } from "../session/SessionProvider";

/**
 * The members that a person can link to, for an owner or an admin (ADR 0069). Anyone else, and a
 * failed load, get nothing, so the form shows no account field; the server checks the right again.
 */
export function useAccounts(api: Api): { id: string; label: string }[] | undefined {
  const manages = canManage(useSession().organization?.role);
  const [accounts, setAccounts] = useState<{ id: string; label: string }[]>();
  useEffect(() => {
    if (!manages) {
      return;
    }
    let current = true;
    api
      .GET("/api/v1/members", { params: { query: { limit: 200 } } })
      .then(({ data }) => {
        if (current && data) {
          setAccounts(
            data.items.map((m) => ({
              id: m.user_id,
              label: m.email ? `${m.display_name} (${m.email})` : m.display_name,
            })),
          );
        }
      })
      .catch(() => undefined);
    return () => {
      current = false;
    };
  }, [api, manages]);
  return manages ? accounts : undefined;
}
