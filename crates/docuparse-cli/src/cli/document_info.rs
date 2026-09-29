use docuparse::PdfMetadata;
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct DocumentInfo<'a> {
    pub page_count: u32,
    pub file_size_bytes: u64,
    pub pdf_version: Option<String>,
    pub title: Option<&'a str>,
    pub author: Option<&'a str>,
    pub subject: Option<&'a str>,
    pub creator: Option<&'a str>,
    pub producer: Option<&'a str>,
}

impl<'a> From<&'a PdfMetadata> for DocumentInfo<'a> {
    fn from(metadata: &'a PdfMetadata) -> Self {
        Self {
            page_count: metadata.page_count,
            file_size_bytes: metadata.file_size_bytes,
            pdf_version: metadata.pdf_version_string(),
            title: metadata.title.as_deref(),
            author: metadata.author.as_deref(),
            subject: metadata.subject.as_deref(),
            creator: metadata.creator.as_deref(),
            producer: metadata.producer.as_deref(),
        }
    }
}
