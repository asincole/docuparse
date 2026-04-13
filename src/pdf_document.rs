use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use pdfium_render::prelude::{PdfPages, Pdfium};

use crate::{
    error::{PdfError, PdfiumError},
    metadata::PdfMetadata,
    validation,
};

pub struct PdfDocument<'pdfium> {
    pdf_doc: pdfium_render::prelude::PdfDocument<'pdfium>,
    pub path: PathBuf,
    pub metadata: PdfMetadata,
}

impl<'pdfium> PdfDocument<'pdfium> {
    fn load(
        pdfium: &'pdfium Pdfium,
        path: &Path,
        password: Option<&str>,
    ) -> Result<Self, PdfError> {
        let path = path.to_path_buf();

        validation::validate_pdf(&path)?;

        let doc =
            pdfium
                .load_pdf_from_file(&path, password)
                .map_err(|err| PdfiumError::LoadFailed {
                    path: path.to_path_buf(),
                    source: Arc::new(err),
                })?;

        let metadata = PdfMetadata::from_doc(&doc, &path);

        Ok(Self {
            pdf_doc: doc,
            path,
            metadata,
        })
    }

    /// Load and validate a PDF, binding it to the given pdfium instance.
    pub fn open(pdfium: &'pdfium Pdfium, path: impl AsRef<Path>) -> Result<Self, PdfError> {
        Self::load(pdfium, path.as_ref(), None)
    }

    /// Open a password-protected PDF.
    pub fn open_with_password(
        pdfium: &'pdfium Pdfium,
        path: impl AsRef<Path>,
        password: &str,
    ) -> Result<Self, PdfError> {
        Self::load(pdfium, path.as_ref(), Some(password))
    }

    pub fn page_count(&self) -> u32 {
        self.metadata.page_count
    }

    pub fn pages(&self) -> &PdfPages<'_> {
        self.pdf_doc.pages()
    }
}
