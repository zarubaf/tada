/**
 * The attribute of a dialog that leaves the page usable behind it, such as the filter popover. A
 * dialog without it is modal: a sheet, a drawer, a confirmation or a picker.
 */
export const MODELESS = "data-modeless";

/** True while a modal dialog is open: then the keys of the page belong to the dialog. */
export function isModalOpen(): boolean {
  return (
    document.querySelector(`[role='dialog']:not([${MODELESS}]), [role='alertdialog']`) !== null
  );
}
