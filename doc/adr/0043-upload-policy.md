# 0043. Upload policy

- Status: Proposed
- Date: 2026-10-06

## Context

Members upload source documents in Slice 1: PDFs, office files, images and plans.
ADR 0009 sends all uploads and downloads through the server and allows inline previews only for PDF, PNG, JPEG and plain text.
A file extension says nothing reliable about the content.
Images from phones can contain location data in their metadata.
A malware scanner such as ClamAV keeps its signature database in memory, more than 1 GB, which is large for a small VM.
`infer` 0.22.0 (July 2026) detects file types from their content.

## Decision

Types:

- The server detects binary types from the content with `infer`, not from the extension or the client's `Content-Type`.
- `infer` cannot detect text formats. A text file (plain text, Markdown or CSV) is accepted only if it is valid UTF-8 without control characters; the extension then selects the text format.
- An allow-list decides what is accepted: PDF, PNG, JPEG, WebP, plain text, Markdown, CSV, the Office Open XML formats (DOCX, XLSX, PPTX), the OpenDocument formats and ZIP archives of these.
- A file whose content does not match an allowed type is rejected with a problem code (ADR 0037).
- An owner can extend the allow-list for the organization, for example for CAD files. Such files are downloads only, never previews.

Size:

- `TADA_UPLOAD_MAX_BYTES` sets the limit for one file. The default is 100 MB.
- Each organization has a storage quota as a setting in the database. An upload above the quota is rejected.

Names:

- tada keeps the original file name as metadata, after it removes control characters and path separators and limits the length.
- The object key never contains the name (ADR 0009).
- Downloads send the name in `Content-Disposition` with the RFC 6266 encoding.

Images:

- tada keeps the original image unchanged as evidence.
- Previews come from a copy without metadata. The location data of the original is visible only to members who can download the original.

Malware:

- tada does not scan uploads in Slice 1.
- The `app` crate defines a `Scanner` port with a "no scan" adapter. An operator can add a ClamAV adapter later through a new ADR.
- Files are never run or opened by tada beyond type detection and preview generation.

## Consequences

- A renamed executable file is rejected.
- The original stays exact evidence, and previews leak no location data.
- Without a scanner, members download files at their own risk, as from any shared drive. The privacy notice and the UI state this.

## Alternatives

- Trust the extension: easy to fake.
- ClamAV from the start: memory cost above the budget of a small VM, for a risk that members' own devices already check.
- Strip metadata from originals: the original would no longer be exact evidence.
