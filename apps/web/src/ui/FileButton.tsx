import { FileTrigger } from "react-aria-components";
import { Button } from "./Button";

export interface FileButtonProps {
  children: string;
  /** The member picked a file. */
  onSelect: (file: File) => void;
  /** The upload runs. The button stays mounted, so that it keeps focus. */
  isPending?: boolean;
}

/** A primary button that opens the file picker (doc/design/components.md, „FileDrop“). */
export function FileButton({ children, onSelect, isPending }: FileButtonProps) {
  return (
    <FileTrigger
      onSelect={(files) => {
        const file = files?.item(0);
        if (file) {
          onSelect(file);
        }
      }}
    >
      <Button variant="primary" isPending={isPending}>
        {children}
      </Button>
    </FileTrigger>
  );
}
