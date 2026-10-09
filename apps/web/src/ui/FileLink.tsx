import buttonStyles from "./Button.module.css";
import linkStyles from "./LinkButton.module.css";

export interface FileLinkProps {
  /** The address of a file on the server. The browser loads it; the router does not. */
  href: string;
  /** Save the file instead of showing it. */
  download?: boolean;
  /** Open the file in a new tab. */
  newTab?: boolean;
  "aria-label"?: string;
  children: string;
}

/**
 * A plain link to a file, in the style of a button. The browser streams the file, so the page
 * never holds it in memory (doc/design/components.md).
 */
export function FileLink({ href, download, newTab, children, ...props }: FileLinkProps) {
  return (
    <a
      {...props}
      href={href}
      download={download || undefined}
      target={newTab ? "_blank" : undefined}
      rel={newTab ? "noopener noreferrer" : undefined}
      className={`${buttonStyles.button} ${linkStyles.link}`}
    >
      {children}
    </a>
  );
}
