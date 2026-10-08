/**
 * The `q` parameter of the document list for the text in the search field. White space at the
 * ends does not count, and a field with only white space is no filter.
 */
export function searchParam(text: string): string | undefined {
  const trimmed = text.trim();
  return trimmed === "" ? undefined : trimmed;
}
