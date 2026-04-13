//! docuparse - PDF loading, rendering and text extraction

mod error;
mod metadata;
mod pdf_document;
mod pdfium_singleton;
pub mod render;
mod text;
mod utils;
mod validation;

pub use error::PdfError;
pub use metadata::PdfMetadata;
pub use pdf_document::PdfDocument;
pub use pdfium_singleton::get_or_init_pdfium as init_pdfium;
pub use render::{RenderConfig, RenderConfigBuilder};
