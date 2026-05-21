#[cfg(feature = "ocr")]
use crate::ocr::OcrBackend;

/// Configuration for opening PDF documents.
///
/// Construct once and pass by reference to [`crate::PdfDocument::load`].
/// `Arc` fields are cloned on each `load` call - no model reload.
///
/// ```rust,no_run
/// # use std::sync::Arc;
/// # use docuparse::PdfOpenConfig;
/// // minimal
/// let config = PdfOpenConfig::builder().build();
///
/// // encrypted
/// let config = PdfOpenConfig::builder()
///     .password("secret".to_owned())
///     .build();
///
/// // with OCR backend
/// # #[cfg(feature = "ocr-onnx")]
/// # {
/// use docuparse::ocr::{OcrBackend, OnnxOcrBackend};
/// let backend = OnnxOcrBackend::from_env().expect("OCR env vars must be set");
/// let config = PdfOpenConfig::builder()
///     .ocr_backend(Arc::new(backend) as Arc<dyn OcrBackend>)
///     .build();
/// # }
/// # Ok::<(), docuparse::LoadError>(())
/// ```
#[derive(Clone, bon::Builder)]
#[non_exhaustive]
pub struct PdfOpenConfig {
    /// Password for encrypted PDFs.
    #[builder(into)]
    pub password: Option<String>,

    /// OCR backend shared across all documents opened with this config.
    /// Only present with the `ocr` feature.
    #[cfg(feature = "ocr")]
    pub ocr_backend: Option<std::sync::Arc<dyn OcrBackend>>,
}
