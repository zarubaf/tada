import { FileTrigger } from "react-aria-components";
import { Button } from "./Button";

export interface FileButtonProps {
  children: string;
  /** The member picked a file. */
  onSelect: (file: File) => void;
  /** The upload runs. The button stays mounted, so that it keeps focus. */
  isPending?: boolean;
  /** `sm` is the compact button of a toolbar row. */
  size?: "md" | "sm";
}

/** A primary button that opens the file picker (doc/design/components.md, „FileDrop“). */
export function FileButton({ children, onSelect, isPending, size }: FileButtonProps) {
  return (
    <FileTrigger
      onSelect={(files) => {
        const file = files?.item(0);
        if (file) {
          onSelect(file);
        }
      }}
    >
      <Button variant="primary" isPending={isPending} {...(size && { size })}>
        {children}
      </Button>
    </FileTrigger>
  );
}
