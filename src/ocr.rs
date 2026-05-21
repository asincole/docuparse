mod backend;
mod document;

use std::{path::PathBuf, sync::Arc};

pub use backend::{OcrBackend, OcrConfig, OcrConfigBuilder, OcrPageResult, OcrPageResultBuilder};

#[cfg(feature = "ocr-onnx")]
mod onnx_backend;
#[cfg(feature = "ocr-onnx")]
pub use onnx_backend::OnnxOcrBackend;
#[cfg(feature = "ocr-openai")]
mod llama_backend;
#[cfg(feature = "ocr-openai")]
pub use llama_backend::{LlamaServerBackend, LlamaServerConfig, LlamaServerConfigBuilder};

/// OCR subsystem errors.
///
/// `source` fields use `Arc<dyn Error + Send + Sync>` rather than `Box` so
/// that `OcrError` remains `Clone`-able and `Send + Sync` - required for
/// rayon workers and `tokio::spawn_blocking`, and to accept errors from
/// third-party [`OcrBackend`] implementations without coupling to this crate's
/// error types.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum OcrError {
    #[error(
        "no OCR backend attached - open the document via \
         `PdfDocument::load` with a `PdfOpenConfig` that includes an `ocr_backend`"
    )]
    BackendNotAttached,

    #[error("failed to render page {page} for OCR")]
    RenderFailed {
        page: u32,
        #[source]
        source: Arc<dyn std::error::Error + Send + Sync>,
    },

    #[error("OCR model artifact not found at `{}`", path.display())]
    ModelNotFound { path: PathBuf },

    #[error(
        "environment variable `{var}` is not set \
         (required: OCR_DET_MODEL_PATH, OCR_REC_MODEL_PATH, OCR_DICT_PATH)"
    )]
    MissingEnvVar { var: &'static str },

    #[error("failed to build OCR backend")]
    BackendBuildFailed {
        #[source]
        source: Arc<dyn std::error::Error + Send + Sync>,
    },

    #[error("OCR inference failed on page {page}")]
    InferenceFailed {
        page: u32,
        #[source]
        source: Arc<dyn std::error::Error + Send + Sync>,
    },

    #[error("image conversion produced a zero-dimension buffer on page {page}")]
    ImageConversionFailed { page: u32 },
}
