//! docuparse - PDF loading, rendering and text extraction

mod metadata;
mod open_config;
mod pdf_document;
mod pdfium_singleton;
mod render;
mod text;
mod utils;
mod validation;

pub use metadata::PdfMetadata;
pub use open_config::{PdfOpenConfig, PdfOpenConfigBuilder};
pub use pdf_document::{LoadError, PdfDocument};
pub use render::{RenderConfig, RenderConfigBuilder, RenderError};
pub use text::TextError;
pub use utils::contains_real_words;
