use std::{path::PathBuf, sync::Arc};

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("file not found: {}", path.display())]
    NotFound { path: PathBuf },

    #[error("path is not a file: {}", path.display())]
    NotAFile { path: PathBuf },

    #[error("file too small to be a valid PDF ({size} bytes, minimum {min} bytes)")]
    TooSmall { size: u64, min: u64 },

    #[error("file too large ({size_mb} MB, maximum {max_mb} MB)")]
    FileTooLarge { size_mb: u64, max_mb: u64 },

    #[error("invalid PDF — missing %PDF- header")]
    InvalidMagicBytes,

    #[error("io error reading `{}`: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("page {page} is out of range (document has {total} pages)")]
    PageOutOfRange { page: u32, total: u32 },

    #[error("failed to render page {page} to bitmap")]
    RenderFailed {
        page: u32,
        #[source]
        source: pdfium_render::prelude::PdfiumError,
    },

    #[error("failed to encode rendered image as {format}")]
    EncodingFailed {
        format: &'static str,
        #[source]
        source: image::ImageError,
    },

    #[error("failed to convert rendered bitmap to image on page {page}")]
    BitmapConversionFailed {
        page: u32,
        #[source]
        source: pdfium_render::prelude::PdfiumError,
    },

    #[error("page {page} has degenerate geometry (width={width_f}, height={height_f})")]
    DegenerateGeometry {
        page: u32,
        width_f: f32,
        height_f: f32,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum TextError {
    #[error("failed to extract text from page {page}")]
    ExtractionFailed {
        page: u32,
        #[source]
        source: pdfium_render::prelude::PdfiumError,
    },
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum PdfiumError {
    #[error("PDFIUM_LIB_PATH environment variable is not set")]
    MissingLibPath,

    #[error("failed to initialize PDFium: {source}")]
    InitFailed {
        #[source]
        source: Arc<pdfium_render::prelude::PdfiumError>,
    },

    #[error("failed to load PDF '{}'", path.display())]
    LoadFailed {
        path: PathBuf,
        #[source]
        source: Arc<pdfium_render::prelude::PdfiumError>,
    },

    #[error("PDFium internal error: {0}")]
    Internal(Arc<dyn std::error::Error + Send + Sync>),
}

/// The sole error type that crosses the library boundary.
/// All domain errors compose into this via `#[from]`.
#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error(transparent)]
    Validation(#[from] ValidationError),

    #[error(transparent)]
    Pdfium(#[from] PdfiumError),

    #[error(transparent)]
    Render(#[from] RenderError),

    #[error(transparent)]
    Text(#[from] TextError),
}
