// The state that the three register pages share: the form to edit, the two live regions and the
// focus targets.
import { useRef, useState } from "react";
import { t } from "../i18n";
import { useFocusAfterCommit, useRetry } from "../ui/focus";
import type { SaveFailure } from "./fieldErrors";

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
    loadMoreFailed: setFailure,
    savedMessage: (name: string) => t("work-saved", { name }),
  };
}
