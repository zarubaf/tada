# 0043. Upload policy

- Status: Accepted
- Date: 2026-10-06

## Context

Members upload source documents in Slice 1: PDFs, office files, images and plans.
ADR 0009 sends all uploads and downloads through the server and allows inline previews only for PDF, PNG, JPEG and plain text.
A file extension says nothing reliable about the content.
Images from phones can contain location data in their metadata.
A malware scanner such as ClamAV keeps its signature database in memory. Its documentation recommends at least 3 GiB of memory, which is large for a small VM.
`infer` 0.22.0 (July 2026) detects file types from their content. It detects the OpenDocument text, spreadsheet and presentation formats only through their uncompressed `mimetype` entry, and it cannot look inside ZIP archives.
`axum` limits a request body to 2 MB by default.

## Decision

Types:

- The server detects binary types from the content with `infer`, not from the extension or the client's `Content-Type`.
- `infer` cannot detect text formats. A text file (plain text, Markdown or CSV) is accepted only if it is valid UTF-8 without control characters; the extension then selects the text format.
- An allow-list decides what is accepted: PDF, PNG, JPEG, WebP, HEIC (the default format of iPhone photos), plain text, Markdown, CSV, the Office Open XML formats (DOCX, XLSX, PPTX) and the OpenDocument formats ODT, ODS and ODP.
- ZIP archives are not accepted. Unpacking them brings the risks of zip bombs and path traversal, and `infer` cannot check their content.
- A file whose content does not match an allowed type is rejected with a problem code (ADR 0037).
- An owner can extend the allow-list for the organization, for example for CAD files. Such files are downloads only, never previews.

Size:

- `TADA_UPLOAD_MAX_BYTES` sets the limit for one file. The default is 100 MB. The upload route raises the body limit of `axum` to this value; other routes keep the default.
- The server streams an upload to its staging key (ADR 0009). It never holds the whole file in memory.
- Each organization has a storage quota as a setting in the database. An upload above the quota is rejected.

Names:

- tada keeps the original file name as metadata, after it removes control characters and path separators and limits the length.
- The object key never contains the name (ADR 0009).
- Downloads send the name in `Content-Disposition` with the RFC 6266 encoding.

Images:

- tada keeps the original image unchanged as evidence.
- Previews come from a copy without metadata. The location data of the original is visible only to members who can download the original.
- The image decoder has limits for the pixel count and the memory, so that a small file cannot expand to a large image in memory.

Malware:

- tada does not scan uploads in Slice 1.
- A scanner, for example ClamAV, can come later through a new ADR. We do not add a port for it now.
- Files are never run or opened by tada beyond type detection and preview generation.

## Consequences

- A renamed executable file is rejected.
- The original stays exact evidence, and previews leak no location data.
- Without a scanner, members download files at their own risk, as from any shared drive. The privacy notice and the UI state this.

## Alternatives

- Trust the extension: easy to fake.
- ClamAV from the start: at least 3 GiB of memory, above the budget of a small VM, for a risk that members' own devices already check.
- A scanner port now with only a "no scan" adapter: a hook for a later need, against YAGNI.
- Strip metadata from originals: the original would no longer be exact evidence.
