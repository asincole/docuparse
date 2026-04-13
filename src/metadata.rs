use std::{ffi::OsStr, fs, path::Path};

use pdfium_render::prelude::{PdfDocument, PdfDocumentMetadataTagType, PdfDocumentVersion};

use crate::utils::{KB, MB};

/// Metadata extracted from a PDF document at load time.
/// Cheap to clone, derived once in `PdfDocument::open`.
#[derive(Debug, Clone)]
pub struct PdfMetadata {
    /// Derived from the file stem — "Test_file.pdf" → "Test_file"
    pub pdf_name: String,

    /// Total number of pages
    pub page_count: u32,

    /// File size in bytes
    pub file_size_bytes: u64,

    /// From the PDF's own metadata dictionary
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub creator: Option<String>,
    pub producer: Option<String>,

    /// PDF spec version e.g. "1.7", "2.0"
    pub pdf_version: PdfDocumentVersion,
}

impl PdfMetadata {
    pub fn from_doc(doc: &PdfDocument, path: &Path) -> Self {
        let pdf_name = path
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or("unknown")
            .to_owned();

        let file_size_bytes = fs::metadata(path).map_or(0, |m| m.len());

        let page_count = doc.pages().len() as u32;

        let pdf_version = doc.version();

        let mut title = None;
        let mut author = None;
        let mut subject = None;
        let mut creator = None;
        let mut producer = None;

        for tag in doc.metadata().iter() {
            let value = non_empty(tag.value());
            match tag.tag_type() {
                PdfDocumentMetadataTagType::Title => title = value,
                PdfDocumentMetadataTagType::Author => author = value,
                PdfDocumentMetadataTagType::Subject => subject = value,
                PdfDocumentMetadataTagType::Creator => creator = value,
                PdfDocumentMetadataTagType::Producer => producer = value,
                // TODO: Keywords, CreationDate, ModificationDate — ignored for now
                _ => {}
            }
        }

        Self {
            pdf_name,
            page_count,
            file_size_bytes,
            title,
            author,
            subject,
            creator,
            producer,
            pdf_version,
        }
    }

    /// Human-readable file size  e.g. "1.7 MB", "340 KB"
    pub fn file_size_display(&self) -> String {
        match self.file_size_bytes {
            bytes if bytes >= MB => format!("{:.1} MB", bytes as f64 / MB as f64),
            bytes if bytes >= KB => format!("{:.1} KB", bytes as f64 / KB as f64),
            bytes => format!("{bytes} B"),
        }
    }
}

/// Treat empty/whitespace-only strings from the PDF dict as absent.
fn non_empty(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn metadata_with_size(file_size_bytes: u64) -> PdfMetadata {
        PdfMetadata {
            pdf_name: String::new(),
            page_count: 0,
            file_size_bytes,
            title: None,
            author: None,
            subject: None,
            creator: None,
            producer: None,
            pdf_version: PdfDocumentVersion::Unset,
        }
    }

    #[rstest]
    #[case::empty("", None)]
    #[case::spaces("   ", None)]
    #[case::tabs_newlines("\t\n\r", None)]
    #[case::plain("Rust", Some("Rust"))]
    #[case::leading_trailing_whitespace("  hello  ", Some("hello"))]
    #[case::newline_padding("\n title \n", Some("title"))]
    #[case::preserves_internal_whitespace("  John   Doe  ", Some("John   Doe"))]
    fn test_non_empty(#[case] input: &str, #[case] expected: Option<&str>) {
        assert_eq!(non_empty(input), expected.map(str::to_owned));
    }

    #[rstest]
    #[case::zero(0, "0 B")]
    #[case::one_byte(1, "1 B")]
    #[case::below_kb(1023, "1023 B")]
    #[case::exact_kb(KB, "1.0 KB")]
    #[case::mid_kb(512 * KB, "512.0 KB")]
    #[case::below_mb(MB - KB, "1023.0 KB")]
    #[case::exact_mb(MB, "1.0 MB")]
    #[case::mid_mb(10 * MB,   "10.0 MB")]
    #[case::max_pdf(500 * MB, "500.0 MB")]
    #[case::one_and_half_mb(MB + MB / 2, "1.5 MB")]
    fn test_file_size_display(#[case] bytes: u64, #[case] expected: &str) {
        assert_eq!(metadata_with_size(bytes).file_size_display(), expected);
    }

    #[rstest]
    #[case::with_extension("Report.pdf", "Report")]
    #[case::with_underscores("Test_file.pdf", "Test_file")]
    #[case::no_extension("myfile", "myfile")]
    #[case::empty_path("", "unknown")]
    fn test_pdf_name_derivation(#[case] path: &str, #[case] expected: &str) {
        let name = Path::new(path)
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or("unknown")
            .to_owned();
        assert_eq!(name, expected);
    }
}
