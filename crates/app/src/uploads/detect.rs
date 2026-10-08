//! Detection of the type of an upload from its content (ADR 0043, ADR 0055).
//!
//! This module is pure and does no I/O.
//! `infer` is a pure crate without I/O, so it is a dependency of `app` and not of an adapter.
//! The upload code keeps the first `SNIFF_BYTES` of the stream for `detect`.
//! It feeds each chunk of the whole stream to a `TextValidator`.

use std::str;

use thiserror::Error;

/// The number of bytes from the start of an upload that `detect` reads (ADR 0055).
pub const SNIFF_BYTES: usize = 64 * 1024;

/// The longest file name that `sanitize_file_name` keeps, in characters.
const MAX_FILE_NAME_CHARS: usize = 200;

/// The signature of a local file header in a ZIP container.
const ZIP_LOCAL_HEADER: &[u8; 4] = b"PK\x03\x04";

/// Flag bit 3: the sizes follow the entry data and the header holds none.
const ZIP_DATA_DESCRIPTOR: usize = 1 << 3;

/// The size of the fixed part of a local file header.
const ZIP_LOCAL_HEADER_LEN: usize = 30;

/// The type of an accepted file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Pdf,
    Png,
    Jpeg,
    Webp,
    Heic,
    Text,
    Markdown,
    Csv,
    Docx,
    Xlsx,
    Pptx,
    Odt,
    Ods,
    Odp,
}

impl FileType {
    /// All types of the allow-list.
    pub const ALL: [Self; 14] = [
        Self::Pdf,
        Self::Png,
        Self::Jpeg,
        Self::Webp,
        Self::Heic,
        Self::Text,
        Self::Markdown,
        Self::Csv,
        Self::Docx,
        Self::Xlsx,
        Self::Pptx,
        Self::Odt,
        Self::Ods,
        Self::Odp,
    ];

    /// The type whose `media_type` is `media_type`, for example of a stored document version.
    pub fn of_media_type(media_type: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|file_type| file_type.media_type() == media_type)
    }

    /// Whether the file is UTF-8 text: plain text, Markdown or CSV.
    pub fn is_text(self) -> bool {
        matches!(self, Self::Text | Self::Markdown | Self::Csv)
    }

    pub fn media_type(self) -> &'static str {
        match self {
            Self::Pdf => "application/pdf",
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
            Self::Heic => "image/heic",
            Self::Text => "text/plain; charset=utf-8",
            Self::Markdown => "text/markdown; charset=utf-8",
            Self::Csv => "text/csv; charset=utf-8",
            Self::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            Self::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            Self::Pptx => {
                "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            }
            Self::Odt => "application/vnd.oasis.opendocument.text",
            Self::Ods => "application/vnd.oasis.opendocument.spreadsheet",
            Self::Odp => "application/vnd.oasis.opendocument.presentation",
        }
    }

    /// Whether the browser may show the file inline.
    /// Images are downloads only, so that no image metadata leaks.
    /// Image previews from copies without metadata (ADR 0043) come later.
    pub fn inline_preview(self) -> bool {
        matches!(self, Self::Pdf | Self::Text)
    }
}

/// The content does not match an allowed type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("the content does not match an allowed file type")]
pub struct Rejected;

/// Detects the type of a file.
///
/// `head` holds the first `SNIFF_BYTES` of the file.
/// `text_valid` is the result of `TextValidator::finish` over the whole stream.
pub fn detect(
    head: &[u8],
    extension: Option<&str>,
    text_valid: bool,
) -> Result<FileType, Rejected> {
    if let Some(kind) = infer::get(head) {
        if kind.mime_type() == "application/zip" {
            return detect_office_zip(head);
        }
        if let Some(file_type) = from_media_type(kind.mime_type()) {
            return Ok(file_type);
        }
        // A short magic number, such as "BM", can match a text file.
        // The text path decides, with the whole-stream check.
    }
    let text_type = match extension.map(str::to_ascii_lowercase).as_deref() {
        Some("txt") => FileType::Text,
        Some("md") => FileType::Markdown,
        Some("csv") => FileType::Csv,
        _ => return Err(Rejected),
    };
    if text_valid {
        Ok(text_type)
    } else {
        Err(Rejected)
    }
}

fn from_media_type(media_type: &str) -> Option<FileType> {
    Some(match media_type {
        "application/pdf" => FileType::Pdf,
        "image/png" => FileType::Png,
        "image/jpeg" => FileType::Jpeg,
        "image/webp" => FileType::Webp,
        "image/heif" | "image/heic" => FileType::Heic,
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => FileType::Docx,
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => FileType::Xlsx,
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => {
            FileType::Pptx
        }
        "application/vnd.oasis.opendocument.text" => FileType::Odt,
        "application/vnd.oasis.opendocument.spreadsheet" => FileType::Ods,
        "application/vnd.oasis.opendocument.presentation" => FileType::Odp,
        _ => return None,
    })
}

/// Walks the local file headers from the start of `head` and reads the entry names (ADR 0055).
/// It never unpacks an entry.
/// It steps from header to header by the sizes in the header.
/// An entry with a data descriptor (flag bit 3) has no size in its header.
/// The walk then stops after that entry, so only the first names decide.
/// A header inside entry data is never read.
fn detect_office_zip(head: &[u8]) -> Result<FileType, Rejected> {
    let mut start = 0;
    while head.get(start..start + ZIP_LOCAL_HEADER.len()) == Some(ZIP_LOCAL_HEADER) {
        let Some(fixed) = head.get(start..start + ZIP_LOCAL_HEADER_LEN) else {
            break;
        };
        let u16_at = |at: usize| usize::from(u16::from_le_bytes([fixed[at], fixed[at + 1]]));
        let flags = u16_at(6);
        let compressed_size = usize::try_from(u32::from_le_bytes([
            fixed[18], fixed[19], fixed[20], fixed[21],
        ]))
        .unwrap_or(usize::MAX);
        let (name_len, extra_len) = (u16_at(26), u16_at(28));
        let name_start = start + ZIP_LOCAL_HEADER_LEN;
        let Some(name) = head.get(name_start..name_start + name_len) else {
            break;
        };
        if name.starts_with(b"word/") {
            return Ok(FileType::Docx);
        }
        if name.starts_with(b"ppt/") {
            return Ok(FileType::Pptx);
        }
        if name.starts_with(b"xl/") {
            return Ok(FileType::Xlsx);
        }
        if flags & ZIP_DATA_DESCRIPTOR != 0 {
            break;
        }
        start = name_start
            .saturating_add(name_len)
            .saturating_add(extra_len)
            .saturating_add(compressed_size);
    }
    Err(Rejected)
}

/// Checks incrementally that a stream is UTF-8 without control characters.
/// Newline, carriage return and tab are allowed.
#[derive(Debug)]
pub struct TextValidator {
    valid: bool,
    /// The start of a multi-byte character that a chunk boundary split.
    pending: Vec<u8>,
}

impl Default for TextValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl TextValidator {
    pub fn new() -> Self {
        Self {
            valid: true,
            pending: Vec::with_capacity(4),
        }
    }

    pub fn feed(&mut self, chunk: &[u8]) {
        let mut rest = chunk;
        while self.valid && !self.pending.is_empty() && !rest.is_empty() {
            self.pending.push(rest[0]);
            rest = &rest[1..];
            match str::from_utf8(&self.pending) {
                Ok(text) => {
                    self.valid = no_control_chars(text);
                    self.pending.clear();
                }
                Err(error) if error.error_len().is_none() => {}
                Err(_) => self.valid = false,
            }
        }
        if !self.valid || !self.pending.is_empty() {
            return;
        }
        match str::from_utf8(rest) {
            Ok(text) => self.valid = no_control_chars(text),
            Err(error) if error.error_len().is_none() => {
                let (done, tail) = rest.split_at(error.valid_up_to());
                self.valid = str::from_utf8(done).is_ok_and(no_control_chars);
                self.pending.extend_from_slice(tail);
            }
            Err(_) => self.valid = false,
        }
    }

    /// True if the whole stream was valid and ended on a character boundary.
    pub fn finish(self) -> bool {
        self.valid && self.pending.is_empty()
    }
}

fn no_control_chars(text: &str) -> bool {
    text.chars()
        .all(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t'))
}

/// The name that replaces an empty name, `.` or `..`.
const FALLBACK_FILE_NAME: &str = "file";

/// Removes control characters, bidirectional controls and path separators.
/// Limits the name to 200 characters.
/// Never returns an empty name, `.` or `..`.
pub fn sanitize_file_name(name: &str) -> String {
    let clean: String = name
        .chars()
        .filter(|c| {
            !c.is_control()
                && !matches!(
                    c,
                    '/' | '\\' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
                )
        })
        .take(MAX_FILE_NAME_CHARS)
        .collect();
    if matches!(clean.as_str(), "" | "." | "..") {
        FALLBACK_FILE_NAME.to_owned()
    } else {
        clean
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stored ZIP entry as a local file header with its name and no content.
    fn zip_entry(name: &str) -> Vec<u8> {
        let mut bytes = ZIP_LOCAL_HEADER.to_vec();
        bytes.extend_from_slice(&[20, 0, 0, 0, 0, 0]); // version, flags, method
        bytes.extend_from_slice(&[0; 16]); // time, date, CRC, sizes
        bytes.extend_from_slice(&u16::try_from(name.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&[0, 0]); // no extra field
        bytes.extend_from_slice(name.as_bytes());
        bytes
    }

    fn zip(names: &[&str]) -> Vec<u8> {
        names.iter().flat_map(|name| zip_entry(name)).collect()
    }

    fn libreoffice_zip(main: &str) -> Vec<u8> {
        zip(&["_rels/.rels", "docProps/app.xml", main])
    }

    fn detect_with(head: &[u8], extension: &str) -> Result<FileType, Rejected> {
        detect(head, Some(extension), true)
    }

    const ELF: &[u8] = b"\x7fELF\x02\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x02\x00\x3e\x00";

    #[test]
    fn detects_docx_with_the_libreoffice_entry_order() {
        let head = libreoffice_zip("word/document.xml");
        assert_eq!(detect_with(&head, "docx"), Ok(FileType::Docx));
    }

    #[test]
    fn detects_pptx_and_xlsx() {
        let pptx = libreoffice_zip("ppt/presentation.xml");
        assert_eq!(detect_with(&pptx, "pptx"), Ok(FileType::Pptx));
        let xlsx = libreoffice_zip("xl/workbook.xml");
        assert_eq!(detect_with(&xlsx, "xlsx"), Ok(FileType::Xlsx));
    }

    #[test]
    fn rejects_a_zip_without_an_office_entry() {
        assert_eq!(detect_with(&zip(&["foo/bar.txt"]), "zip"), Err(Rejected));
    }

    #[test]
    fn rejects_docx_extension_with_other_zip_content() {
        assert_eq!(detect_with(&zip(&["foo/bar.txt"]), "docx"), Err(Rejected));
    }

    #[test]
    fn rejects_a_renamed_executable() {
        assert_eq!(detect_with(ELF, "pdf"), Err(Rejected));
    }

    #[test]
    fn rejects_a_binary_file_with_a_text_extension() {
        let mut validator = TextValidator::new();
        validator.feed(ELF);
        assert_eq!(detect(ELF, Some("txt"), validator.finish()), Err(Rejected));
    }

    #[test]
    fn rejects_text_with_a_nul_byte() {
        let mut validator = TextValidator::new();
        validator.feed(b"hello\0world");
        assert_eq!(
            detect(b"hello\0world", Some("txt"), validator.finish()),
            Err(Rejected)
        );
    }

    #[test]
    fn accepts_text_formats_by_extension() {
        assert_eq!(
            detect(b"# Title\n", Some("md"), true),
            Ok(FileType::Markdown)
        );
        assert_eq!(
            detect(b"a,b\n1,2\r\n", Some("CSV"), true),
            Ok(FileType::Csv)
        );
        assert_eq!(
            detect(b"hello\tworld\n", Some("txt"), true),
            Ok(FileType::Text)
        );
    }

    #[test]
    fn rejects_text_without_a_text_extension() {
        assert_eq!(detect(b"hello", None, true), Err(Rejected));
        assert_eq!(detect(b"hello", Some("exe"), true), Err(Rejected));
    }

    #[test]
    fn rejects_a_zip_entry_name_beyond_the_sniffed_bytes() {
        let mut bytes = zip(&["_rels/.rels"]);
        bytes.resize(SNIFF_BYTES, 0);
        bytes.extend_from_slice(&zip_entry("word/document.xml"));
        assert_eq!(detect_with(&bytes[..SNIFF_BYTES], "docx"), Err(Rejected));
    }

    #[test]
    fn detects_binary_types_from_their_headers() {
        let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01";
        assert_eq!(detect_with(png, "png"), Ok(FileType::Png));
        let jpeg = b"\xff\xd8\xff\xe0\0\x10JFIF\0\x01\x01";
        assert_eq!(detect_with(jpeg, "jpg"), Ok(FileType::Jpeg));
        assert_eq!(
            detect_with(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n", "pdf"),
            Ok(FileType::Pdf)
        );
    }

    #[test]
    fn previews_inline_only_pdf_and_text() {
        use FileType::*;
        let all = [
            Pdf, Png, Jpeg, Webp, Heic, Text, Markdown, Csv, Docx, Xlsx, Pptx, Odt, Ods, Odp,
        ];
        for file_type in all {
            assert_eq!(file_type.inline_preview(), matches!(file_type, Pdf | Text));
        }
    }

    #[test]
    fn validator_accepts_a_character_split_across_chunks() {
        let bytes = "Zürich €".as_bytes();
        for split in 0..=bytes.len() {
            let mut validator = TextValidator::new();
            validator.feed(&bytes[..split]);
            validator.feed(&bytes[split..]);
            assert!(validator.finish(), "split at {split}");
        }
    }

    #[test]
    fn validator_rejects_invalid_utf8_and_a_truncated_character() {
        let mut invalid = TextValidator::new();
        invalid.feed(b"ab\xffcd");
        assert!(!invalid.finish());
        let mut truncated = TextValidator::new();
        truncated.feed(&"€".as_bytes()[..2]);
        assert!(!truncated.finish());
    }

    #[test]
    fn sanitizes_file_names() {
        assert_eq!(sanitize_file_name("../a\\b/c\u{0}d\n.pdf"), "..abcd.pdf");
        assert_eq!(sanitize_file_name(&"x".repeat(300)).chars().count(), 200);
    }

    #[test]
    fn rejects_a_fake_header_inside_entry_data() {
        // A stored entry whose data holds a header named "word/x".
        let fake = zip_entry("word/x");
        let mut entry = zip_entry("foo/bar.txt");
        entry[18..22].copy_from_slice(&u32::try_from(fake.len()).unwrap().to_le_bytes());
        entry.extend_from_slice(&fake);
        assert_eq!(detect_with(&entry, "docx"), Err(Rejected));
    }

    #[test]
    fn steps_over_entry_data_to_the_next_header() {
        let mut first = zip_entry("_rels/.rels");
        first[18..22].copy_from_slice(&5u32.to_le_bytes());
        first.extend_from_slice(b"hello");
        first.extend_from_slice(&zip_entry("word/document.xml"));
        assert_eq!(detect_with(&first, "docx"), Ok(FileType::Docx));
    }

    #[test]
    fn reads_the_first_name_of_an_entry_with_a_data_descriptor() {
        let mut first = zip_entry("word/document.xml");
        first[6] = 8;
        assert_eq!(detect_with(&first, "docx"), Ok(FileType::Docx));
        let mut other = zip_entry("foo.txt");
        other[6] = 8;
        other.extend_from_slice(&zip_entry("word/document.xml"));
        assert_eq!(detect_with(&other, "docx"), Err(Rejected));
    }

    #[test]
    fn accepts_text_that_starts_like_a_bitmap_header() {
        let mut validator = TextValidator::new();
        validator.feed(b"BMW\n");
        assert_eq!(
            detect(b"BMW\n", Some("txt"), validator.finish()),
            Ok(FileType::Text)
        );
        assert_eq!(detect(b"BMW\n", Some("png"), true), Err(Rejected));
    }

    #[test]
    fn detects_webp_heic_and_opendocument() {
        let webp = b"RIFF\x24\0\0\0WEBPVP8 ";
        assert_eq!(detect_with(webp, "webp"), Ok(FileType::Webp));
        let heic = b"\0\0\0\x18ftypheic\0\0\0\0mif1heic";
        assert_eq!(detect_with(heic, "heic"), Ok(FileType::Heic));
        let mut odt = zip_entry("mimetype");
        let mime = b"application/vnd.oasis.opendocument.text";
        odt[18..22].copy_from_slice(&u32::try_from(mime.len()).unwrap().to_le_bytes());
        odt.extend_from_slice(mime);
        assert_eq!(detect_with(&odt, "odt"), Ok(FileType::Odt));
    }

    #[test]
    fn validator_rejects_a_c1_control_completed_from_pending() {
        let mut validator = TextValidator::new();
        validator.feed(&[0xC2]);
        validator.feed(&[0x85]); // U+0085
        assert!(!validator.finish());
    }

    #[test]
    fn validator_accepts_one_byte_at_a_time() {
        let mut validator = TextValidator::new();
        for byte in "Zürich €\n".as_bytes() {
            validator.feed(&[*byte]);
        }
        assert!(validator.finish());
        assert!(TextValidator::default().finish());
    }

    #[test]
    fn sanitize_removes_bidi_controls_and_never_returns_a_bare_dot_name() {
        assert_eq!(sanitize_file_name("a\u{202E}fdp.exe\u{2066}"), "afdp.exe");
        for name in ["", ".", "..", "/", "\n"] {
            assert_eq!(sanitize_file_name(name), "file");
        }
    }

    #[test]
    fn each_type_comes_back_from_its_media_type() {
        // A new type fails to compile here until it is in this match and in `ALL`.
        let in_all = |file_type: FileType| match file_type {
            FileType::Pdf
            | FileType::Png
            | FileType::Jpeg
            | FileType::Webp
            | FileType::Heic
            | FileType::Text
            | FileType::Markdown
            | FileType::Csv
            | FileType::Docx
            | FileType::Xlsx
            | FileType::Pptx
            | FileType::Odt
            | FileType::Ods
            | FileType::Odp => FileType::ALL.contains(&file_type),
        };
        assert!(FileType::ALL.into_iter().all(in_all));
        assert_eq!(FileType::ALL.len(), 14);
        for file_type in FileType::ALL {
            assert_eq!(
                FileType::of_media_type(file_type.media_type()),
                Some(file_type)
            );
        }
        assert_eq!(FileType::of_media_type("application/zip"), None);
    }
}
