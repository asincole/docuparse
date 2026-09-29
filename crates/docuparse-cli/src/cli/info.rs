use docuparse::{PdfDocument, PdfOpenConfig};
use usage_rs::{Args, Run};

use super::error::fail;

/// Show document metadata: page count, file size, title, author, PDF version.
#[derive(Args)]
pub(crate) struct Info {
    /// Path to the PDF file
    path: String,

    /// Print machine-readable JSON instead of a summary
    #[usage(long)]
    json: bool,
}

impl Run for Info {
    type Output = ();

    fn run(self) {
        let doc = PdfDocument::load(&self.path, &PdfOpenConfig::builder().build())
            .unwrap_or_else(|e| fail(e));

        if self.json {
            println!(
                "{}",
                serde_json::json!({
                    "pdf_name": doc.metadata.pdf_name,
                    "page_count": doc.metadata.page_count,
                    "file_size_bytes": doc.metadata.file_size_bytes,
                    "title": doc.metadata.title,
                    "author": doc.metadata.author,
                    "subject": doc.metadata.subject,
                    "creator": doc.metadata.creator,
                    "producer": doc.metadata.producer,
                    "pdf_version": format!("{:?}", doc.metadata.pdf_version),
                })
            );
        } else {
            println!("name        {}", doc.metadata.pdf_name);
            println!("pages       {}", doc.metadata.page_count);
            println!("size        {}", doc.metadata.file_size_display());
            println!(
                "title       {}",
                doc.metadata.title.as_deref().unwrap_or("-")
            );
            println!(
                "author      {}",
                doc.metadata.author.as_deref().unwrap_or("-")
            );
            println!(
                "subject     {}",
                doc.metadata.subject.as_deref().unwrap_or("-")
            );
            println!(
                "creator     {}",
                doc.metadata.creator.as_deref().unwrap_or("-")
            );
            println!(
                "producer    {}",
                doc.metadata.producer.as_deref().unwrap_or("-")
            );
            println!("pdf version {:?}", doc.metadata.pdf_version);
        }
    }
}
