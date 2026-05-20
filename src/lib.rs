#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod metadata;
#[cfg(feature = "ocr")]
pub mod ocr;
mod open_config;
mod pdf_document;
mod pdfium_singleton;
mod render;
mod text;
mod utils;
mod validation;

pub use metadata::PdfMetadata;
#[cfg(feature = "ocr")]
pub use ocr::{
    OcrBackend, OcrConfig, OcrConfigBuilder, OcrError, OcrPageResult, OcrPageResultBuilder,
};
pub use open_config::{PdfOpenConfig, PdfOpenConfigBuilder};
pub use pdf_document::{LoadError, PdfDocument};
pub use render::{RenderConfig, RenderConfigBuilder, RenderError};
pub use text::TextError;
pub use utils::contains_real_words;
