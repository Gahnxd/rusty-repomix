//! Binary filtering and character-set normalization for file contents.
//!
//! This ports the behavior of `repomix/src/core/file/fileRead.ts`: binary
//! extensions are rejected before I/O, UTF-8 is the fast path, BOM-marked
//! UTF-16/32 content is allowed through NULL-byte checks, and legacy text is
//! decoded only after binary inspection.

use std::path::Path;

use chardetng::{EncodingDetector, Iso2022JpDetection, Utf8Detection};
use content_inspector::{inspect, ContentType};
use encoding_rs::{Encoding, UTF_16BE, UTF_16LE};

use super::process_constants::BINARY_EXTENSIONS;

/// Why a discovered file was not admitted to the packed text input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileSkipReason {
    BinaryExtension,
    BinaryContent,
    SizeLimit,
    EncodingError,
}

/// The normalized result of reading one file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileReadResult {
    pub content: Option<String>,
    pub skipped_reason: Option<FileSkipReason>,
    pub size_bytes: usize,
}

impl FileReadResult {
    pub fn text(content: String, size_bytes: usize) -> Self {
        Self {
            content: Some(content),
            skipped_reason: None,
            size_bytes,
        }
    }

    pub fn skipped(reason: FileSkipReason, size_bytes: usize) -> Self {
        Self {
            content: None,
            skipped_reason: Some(reason),
            size_bytes,
        }
    }
}

/// Returns whether a path has a well-known binary-file extension.
///
/// The TypeScript implementation delegates this to `is-binary-path`. This
/// focused native table covers its common repository asset and executable
/// classes; content inspection remains the authoritative fallback for unknown
/// extensions.
pub fn is_binary_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            BINARY_EXTENSIONS
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        })
}

/// Converts already-read bytes to UTF-8 or reports a TypeScript-compatible skip reason.
pub fn process_file_bytes(bytes: &[u8]) -> FileReadResult {
    if !has_text_bom(bytes) && bytes.contains(&0) {
        return FileReadResult::skipped(FileSkipReason::BinaryContent, bytes.len());
    }

    // `content_inspector` supplements the NULL-byte probe with format magic
    // checks (notably PDF and PNG), catching binary payloads that are valid
    // UTF-8 by coincidence and use an unfamiliar extension.
    if inspect(bytes).is_binary() {
        return FileReadResult::skipped(FileSkipReason::BinaryContent, bytes.len());
    }

    if let Ok(content) = std::str::from_utf8(bytes) {
        return FileReadResult::text(
            content.strip_prefix('\u{feff}').unwrap_or(content).into(),
            bytes.len(),
        );
    }

    match inspect(bytes) {
        ContentType::BINARY => FileReadResult::skipped(FileSkipReason::BinaryContent, bytes.len()),
        ContentType::UTF_16LE => {
            decode_with_encoding(UTF_16LE, bytes.get(2..).unwrap_or_default(), bytes.len())
        }
        ContentType::UTF_16BE => {
            decode_with_encoding(UTF_16BE, bytes.get(2..).unwrap_or_default(), bytes.len())
        }
        ContentType::UTF_32LE => {
            decode_utf32(bytes.get(4..).unwrap_or_default(), true, bytes.len())
        }
        ContentType::UTF_32BE => {
            decode_utf32(bytes.get(4..).unwrap_or_default(), false, bytes.len())
        }
        ContentType::UTF_8 | ContentType::UTF_8_BOM => detect_and_decode_legacy(bytes),
    }
}

fn has_text_bom(bytes: &[u8]) -> bool {
    matches!(
        bytes,
        [0xef, 0xbb, 0xbf, ..]
            | [0x00, 0x00, 0xfe, 0xff, ..]
            | [0xff, 0xfe, 0x00, 0x00, ..]
            | [0x84, 0x31, 0x95, 0x33, ..]
            | [0xfe, 0xff, ..]
            | [0xff, 0xfe, ..]
    )
}

fn detect_and_decode_legacy(bytes: &[u8]) -> FileReadResult {
    let mut detector = EncodingDetector::new(Iso2022JpDetection::Allow);
    detector.feed(bytes, true);
    let encoding = detector.guess(None, Utf8Detection::Allow);
    decode_with_encoding(encoding, bytes, bytes.len())
}

fn decode_with_encoding(
    encoding: &'static Encoding,
    bytes: &[u8],
    size_bytes: usize,
) -> FileReadResult {
    let (content, had_errors) = encoding.decode_without_bom_handling(bytes);
    if had_errors {
        return FileReadResult::skipped(FileSkipReason::EncodingError, size_bytes);
    }
    FileReadResult::text(content.into_owned(), size_bytes)
}

fn decode_utf32(bytes: &[u8], little_endian: bool, size_bytes: usize) -> FileReadResult {
    if !bytes.len().is_multiple_of(4) {
        return FileReadResult::skipped(FileSkipReason::EncodingError, size_bytes);
    }

    let mut content = String::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(4) {
        let code_point = if little_endian {
            u32::from_le_bytes(chunk.try_into().unwrap_or_default())
        } else {
            u32::from_be_bytes(chunk.try_into().unwrap_or_default())
        };
        let Some(character) = char::from_u32(code_point) else {
            return FileReadResult::skipped(FileSkipReason::EncodingError, size_bytes);
        };
        content.push(character);
    }
    FileReadResult::text(content, size_bytes)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use encoding_rs::{SHIFT_JIS, WINDOWS_1252};

    use super::{is_binary_path, process_file_bytes, FileSkipReason};

    #[test]
    fn rejects_common_binary_extensions_before_the_read_layer() {
        assert!(is_binary_path(Path::new("assets/logo.PNG")));
        assert!(is_binary_path(Path::new("program.exe")));
        assert!(is_binary_path(Path::new("manual.pdf")));
        assert!(is_binary_path(Path::new("archive.zip")));
        assert!(!is_binary_path(Path::new("src/lib.rs")));
    }

    #[test]
    fn rejects_null_bytes_without_a_text_bom() {
        let result = process_file_bytes(b"hello\0world");

        assert_eq!(result.content, None);
        assert_eq!(result.skipped_reason, Some(FileSkipReason::BinaryContent));
    }

    #[test]
    fn rejects_pdf_magic_even_with_an_unfamiliar_extension() {
        let result = process_file_bytes(b"%PDF-1.7\n");

        assert_eq!(result.skipped_reason, Some(FileSkipReason::BinaryContent));
    }

    #[test]
    fn strips_utf8_bom_without_changing_crlf_line_endings() {
        let result = process_file_bytes(b"\xef\xbb\xbfalpha\r\nbeta\r\n");

        assert_eq!(result.content.as_deref(), Some("alpha\r\nbeta\r\n"));
    }

    #[test]
    fn decodes_utf16_le_bom_with_embedded_null_bytes() {
        let mut bytes = vec![0xff, 0xfe];
        bytes.extend("Hello\r\n".encode_utf16().flat_map(u16::to_le_bytes));

        let result = process_file_bytes(&bytes);

        assert_eq!(result.content.as_deref(), Some("Hello\r\n"));
    }

    #[test]
    fn detects_and_decodes_shift_jis_and_windows_1252() {
        let (shift_jis, _, shift_jis_errors) = SHIFT_JIS.encode("こんにちは\r\n");
        assert!(!shift_jis_errors);
        let shift_jis_result = process_file_bytes(&shift_jis);
        assert_eq!(shift_jis_result.content.as_deref(), Some("こんにちは\r\n"));

        let (windows_1252, _, windows_1252_errors) = WINDOWS_1252.encode("café\n");
        assert!(!windows_1252_errors);
        let windows_1252_result = process_file_bytes(&windows_1252);
        assert_eq!(windows_1252_result.content.as_deref(), Some("café\n"));
    }

    #[test]
    fn rejects_invalid_utf32_code_points() {
        let bytes = [0xff, 0xfe, 0x00, 0x00, 0x00, 0x00, 0x11, 0x00];
        let result = process_file_bytes(&bytes);

        assert_eq!(result.skipped_reason, Some(FileSkipReason::EncodingError));
    }
}
