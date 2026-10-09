// The state that the register pages share: the form to edit, the two live regions and the focus
// targets.
import { useRef, useState } from "react";
import { t } from "../i18n";
import { useFocusAfterCommit, useRetry } from "../ui/focus";
import type { SaveFailure } from "./saveFailure";
import type { Register } from "./useRegister";

/** What `RegisterView` needs of the state of the page. */
export type RegisterPaging = Pick<
  ReturnType<typeof useRegisterPage>,
  "retry" | "retried" | "loadMore"
>;

export function useRegisterPage<T>(reload: () => Promise<boolean>) {
  const [editing, setEditing] = useState<T>();
  const [failure, setFailure] = useState<string>();
  const [confirmation, setConfirmation] = useState<string>();
  const heading = useRef<HTMLHeadingElement>(null);
  const formHeading = useRef<HTMLHeadingElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  const { retried, retry } = useRetry(() => heading.current);

  /** A new action of the member clears the messages of the last one. */
  const start = () => {
    setFailure(undefined);
    setConfirmation(undefined);
  };

  return {
    editing,
    failure,
    confirmation,
    heading,
    formHeading,
    retried,
    retry,
    start,
    setFailure,
    setConfirmation,
    /** Opens the form with a record and moves focus to its heading. */
    edit: (record: T) => {
      start();
      setEditing(record);
      focusAfterCommit(() => formHeading.current);
    },
    /** Closes the form and moves focus to the heading of the page. */
    closeForm: () => {
      setEditing(undefined);
      focusAfterCommit(() => heading.current);
    },
    /** A failed save. A version conflict closes the form and loads the rows again. */
    fail: ({ message, conflict }: SaveFailure) => {
      setFailure(message);
      if (conflict) {
        setEditing(undefined);
        void reload();
      }
    },
    /** Loads the next page, tells the member and keeps focus when the button goes away. */
    loadMore: async (register: Pick<Register<{ id: string }>, "loadMore">) => {
      start();
      const result = await register.loadMore();
      if ("failure" in result) {
        setFailure(result.failure);
        return;
      }
      setConfirmation(t("register-loaded-more"));
      if (result.last) {
        focusAfterCommit(() => heading.current);
      }
    },
    savedMessage: (name: string) => t("register-saved", { name }),
  };
}
