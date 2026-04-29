use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use pdfium_render::prelude::PdfPages;

use crate::{
    error::{PdfError, PdfiumInitError},
    metadata::PdfMetadata,
    open_config::PdfOpenConfig,
    pdfium_singleton::get_or_init_pdfium,
    validation,
};

/// An open, validated PDF document.
///
/// Backed by a process-global `'static` pdfium binding - storable in structs
/// and sendable across threads without lifetime annotations.
pub struct PdfDocument {
    pdf_doc: pdfium_render::prelude::PdfDocument<'static>,
    pub path: PathBuf,
    pub metadata: PdfMetadata,
}

impl PdfDocument {
    /// Load and validate a PDF from `path` using `config`.
    ///
    /// ```rust,no_run
    /// # use pdf_parser::{PdfDocument, open_config::PdfOpenConfig};
    /// let doc = PdfDocument::load("document.pdf", &PdfOpenConfig::builder().build())?;
    /// # Ok::<(), pdf_parser::error::PdfError>(())
    /// ```
    pub fn load(path: impl AsRef<Path>, config: &PdfOpenConfig) -> Result<Self, PdfError> {
        let path = path.as_ref().to_path_buf();
        validation::validate_pdf(&path)?;

        let pdfium = get_or_init_pdfium()?;

        let doc = pdfium
            .load_pdf_from_file(&path, config.password.as_deref())
            .map_err(|err| PdfiumInitError::LoadFailed {
                path: path.clone(),
                source: Arc::new(err),
            })?;

        let metadata = PdfMetadata::from_doc(&doc, &path);

        Ok(Self {
            pdf_doc: doc,
            path,
            metadata,
        })
    }

    pub fn page_count(&self) -> u32 {
        self.metadata.page_count
    }

    pub fn pages(&self) -> &PdfPages<'_> {
        self.pdf_doc.pages()
    }
}
