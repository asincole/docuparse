/// Configuration for opening PDF documents.
///
/// Construct once and pass by reference to [`PdfDocument::load`].
/// `Arc` fields are cloned on each `load` call - no model reload.
///
/// ```rust,no_run
/// # use std::sync::Arc;
/// # use pdf_parser::open_config::PdfOpenConfig;
/// // minimal
/// let config = PdfOpenConfig::builder().build();
///
/// // encrypted
/// let config = PdfOpenConfig::builder()
///     .password("secret".to_owned())
///     .build();
/// # Ok::<(), pdf_parser::error::PdfError>(())
/// ```
#[derive(Clone, bon::Builder)]
#[non_exhaustive]
pub struct PdfOpenConfig {
    /// Password for encrypted PDFs.
    #[builder(into)]
    pub password: Option<String>,
}
