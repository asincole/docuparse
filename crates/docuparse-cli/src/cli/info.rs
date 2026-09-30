use std::io::{self, BufWriter, Write};

use docuparse::{PdfDocument, PdfOpenConfig};
use serde::Serialize;
use usage_rs::{Args, Run};

use super::{document_info::DocumentInfo, error::fail};

#[derive(Serialize)]
struct InfoOutput<'a> {
    pdf_name: &'a str,
    #[serde(flatten)]
    document: DocumentInfo<'a>,
}

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
        let metadata = &doc.metadata;

        let stdout = io::stdout();
        let mut output = BufWriter::new(stdout.lock());

        let result = if self.json {
            let mut document = DocumentInfo::from(metadata);
            document.pdf_version = Some(format!("{:?}", metadata.pdf_version));
            let value = InfoOutput {
                pdf_name: &metadata.pdf_name,
                document,
            };

            serde_json::to_value(&value)
                .and_then(|value| serde_json::to_writer(&mut output, &value))
                .map_err(io::Error::other)
                .and_then(|()| writeln!(output))
        } else {
            writeln!(
                output,
                concat!(
                    "name        {}\n",
                    "pages       {}\n",
                    "size        {}\n",
                    "title       {}\n",
                    "author      {}\n",
                    "subject     {}\n",
                    "creator     {}\n",
                    "producer    {}\n",
                    "pdf version {:?}",
                ),
                metadata.pdf_name,
                metadata.page_count,
                metadata.file_size_display(),
                metadata.title.as_deref().unwrap_or("-"),
                metadata.author.as_deref().unwrap_or("-"),
                metadata.subject.as_deref().unwrap_or("-"),
                metadata.creator.as_deref().unwrap_or("-"),
                metadata.producer.as_deref().unwrap_or("-"),
                metadata.pdf_version,
            )
        };

        result
            .and_then(|()| output.flush())
            .unwrap_or_else(|e| fail(e));
    }
}
