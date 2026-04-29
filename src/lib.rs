//! docuparse - PDF loading, rendering and text extraction

mod error;
mod metadata;
mod open_config;
mod pdf_document;
mod pdfium_singleton;
pub mod render;
mod text;
mod utils;
mod validation;

pub use error::PdfError;
pub use metadata::PdfMetadata;
pub use open_config::PdfOpenConfig;
pub use pdf_document::PdfDocument;
pub use render::{RenderConfig, RenderConfigBuilder};
pub use utils::contains_real_words;
