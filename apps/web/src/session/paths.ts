// The paths of the pages that a member without a session can open. The magic-link and invitation
// paths equal MAGIC_LINK_PATH and INVITATION_PATH of the server (crates/app/src/public_url.rs):
// the mails link to them.
export const SIGN_IN_PATH = "/sign-in";
export const MAGIC_LINK_PATH = "/sign-in/link";
export const INVITATION_PATH = "/invitation";
export const CHOOSE_ORGANIZATION_PATH = "/choose-organization";

const PUBLIC_PATHS = [SIGN_IN_PATH, MAGIC_LINK_PATH, INVITATION_PATH];

export function isPublicPath(pathname: string): boolean {
  return PUBLIC_PATHS.includes(pathname);
}
