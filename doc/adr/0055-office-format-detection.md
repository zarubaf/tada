# 0055. Detection of office formats in ZIP containers

- Status: Accepted
- Date: 2026-10-06
- Amends: [0043](0043-upload-policy.md)

## Context

ADR 0043 detects binary types from the content with `infer` 0.22.0.
The upload spike of Slice 0 tested `infer` on 2026-10-06 with files of each allowed type from LibreOffice 26.8, ImageMagick and `heif-enc`:

| Sample          | Result of `infer` 0.22.0                   | Bytes that the result needs |
| --------------- | ------------------------------------------ | --------------------------- |
| PDF             | `application/pdf`                          | 17                          |
| PNG, JPEG, WebP | correct type                               | 17                          |
| HEIC            | `image/heif`                               | 33                          |
| XLSX            | correct type                               | 33                          |
| ODT, ODS, ODP   | correct type                               | 81 to 97                    |
| DOCX, PPTX      | `application/zip`, also with the full file | not detected                |
| Plain text, CSV | no type, as ADR 0043 expects               | not applicable              |

`infer` checks the first ZIP entries by their position only.
LibreOffice writes `_rels/.rels` and `docProps/` first, and an early return in `infer` stops before the entry that names the format.
The entries that name the format were within the first 3.8 KB of each sample: `word/` for DOCX, `ppt/` for PPTX and `xl/` for XLSX.

## Decision

- The server keeps `infer` for all types except the Office Open XML formats.
- If `infer` reports a ZIP container, the server reads the names of the ZIP entries in the first 64 KiB of the upload:
  - an entry name that starts with `word/` gives DOCX,
  - `ppt/` gives PPTX,
  - `xl/` gives XLSX.
- The scan reads only the entry names of the local file headers. It never unpacks an entry.
- The server rejects a ZIP container without such an entry in the first 64 KiB, as ADR 0043 rejects other ZIP archives.
- The upload code keeps the first 64 KiB for this check while it streams the file to the staging key (ADR 0009).
- Tests use files that the test code builds at run time, so that no office file with author metadata enters the repository.

## Consequences

- The server accepts DOCX and PPTX files from LibreOffice and other producers.
- The server still rejects a file with the extension `.docx` and other ZIP content.
- We own a small scanner of about 40 lines and its tests.

## Alternatives

- `infer` alone: it rejects valid DOCX and PPTX files.
- The ZIP central directory at the end of the file: exact for any entry order, but it needs a second read of the staged object.
- A fix in `infer`: useful, and we can propose it upstream, but tada cannot wait for a release.
- Trust the extension for ZIP containers: ADR 0043 rejects this.
