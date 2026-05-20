use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use pdfium_render::prelude::PdfPages;

use crate::{
    metadata::PdfMetadata,
    open_config::PdfOpenConfig,
    pdfium_singleton::{InitError, get_or_init_pdfium},
    validation,
};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LoadError {
    #[error("PDFIUM_LIB_PATH environment variable is not set")]
    PdfiumLibPathNotSet,

    #[error("PDFium library could not be initialised")]
    PdfiumInit {
        #[source]
        source: Arc<dyn std::error::Error + Send + Sync>,
    },

    #[error("failed to load PDF '{}'", path.display())]
    ParseFailed {
        path: PathBuf,
        #[source]
        source: Arc<dyn std::error::Error + Send + Sync>,
    },

    #[error("file not found: {}", path.display())]
    NotFound { path: PathBuf },

    #[error("path is not a file: {}", path.display())]
    NotAFile { path: PathBuf },

    #[error("file too small to be a valid PDF ({size} bytes, minimum {min} bytes)")]
    TooSmall { size: u64, min: u64 },

    #[error("file too large ({size_mb} MB, maximum {max_mb} MB)")]
    FileTooLarge { size_mb: u64, max_mb: u64 },

    #[error("invalid PDF - missing %PDF- header")]
    InvalidMagicBytes,

    #[error("io error reading '{}': {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// An open, validated PDF document.
///
/// Backed by a process-global `'static` pdfium binding - storable in structs
/// and sendable across threads without lifetime annotations.
pub struct PdfDocument {
    pdf_doc: pdfium_render::prelude::PdfDocument<'static>,
    pub path: PathBuf,
    pub metadata: PdfMetadata,

    #[cfg(feature = "ocr")]
    pub(crate) ocr_backend: Option<Arc<dyn crate::ocr::OcrBackend>>,
}

#[cfg_attr(feature = "hotpath", hotpath::measure_all)]
impl PdfDocument {
    /// Load and validate a PDF from `path` using `config`.
    ///
    /// ```rust,no_run
    /// # use docuparse::{PdfDocument, PdfOpenConfig};
    /// let doc = PdfDocument::load("document.pdf", &PdfOpenConfig::builder().build())?;
    /// # Ok::<(), docuparse::LoadError>(())
    /// ```
    pub fn load(path: impl AsRef<Path>, config: &PdfOpenConfig) -> Result<Self, LoadError> {
        let path = path.as_ref().to_path_buf();
        let file = validation::validate_pdf(&path)?;

        let pdfium = get_or_init_pdfium().map_err(|e| match e {
            InitError::MissingLibPath => LoadError::PdfiumLibPathNotSet,
            InitError::BindFailed { source } => LoadError::PdfiumInit { source },
        })?;

        let doc = pdfium
            .load_pdf_from_reader(file, config.password.as_deref())
            .map_err(|err| LoadError::ParseFailed {
                path: path.clone(),
                source: Arc::new(err),
            })?;

        let metadata = PdfMetadata::from_doc(&doc, &path);

        Ok(Self {
            pdf_doc: doc,
            path,
            metadata,
            #[cfg(feature = "ocr")]
            ocr_backend: config.ocr_backend.clone(),
        })
    }

    pub fn page_count(&self) -> u32 {
        self.metadata.page_count
    }

    pub fn pages(&self) -> &PdfPages<'_> {
        self.pdf_doc.pages()
    }

    /// Returns the attached OCR backend, if any.
    #[cfg(feature = "ocr")]
    #[inline]
    pub fn ocr_backend(&self) -> Option<&dyn crate::ocr::OcrBackend> {
        self.ocr_backend.as_deref()
    }

    #[cfg(feature = "ocr")]
    #[inline]
    pub(crate) fn require_ocr_backend(
        &self,
    ) -> Result<&dyn crate::ocr::OcrBackend, crate::ocr::OcrError> {
        self.ocr_backend
            .as_deref()
            .ok_or_else(|| crate::ocr::OcrError::BackendNotAttached.into())
    }
}
