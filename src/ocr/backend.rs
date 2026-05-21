use image::DynamicImage;

/// OCR output for a single page. Backend-agnostic.
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
pub struct OcrPageResult {
    /// Zero-based page index within the source document.
    pub page_index: u32,

    /// Confidence-filtered plain text, newline-joined. Empty if no regions passed the threshold.
    pub text: String,
}

impl OcrPageResult {
    /// Returns `true` if any text survived the confidence filter.
    #[inline]
    pub fn has_text(&self) -> bool {
        !self.text.is_empty()
    }
}

/// Per-call OCR configuration.
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
pub struct OcrConfig {
    /// Minimum confidence \[0.0, 1.0\] for a region to be included in output.
    #[builder(default = 0.7)]
    pub confidence_threshold: f32,
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self::builder().build()
    }
}

/// Pluggable OCR inference backend.
///
/// Implementations must be `Send + Sync` - backends are shared across rayon
/// threads via `Arc<dyn OcrBackend>`. The backend owns all format conversion
/// (e.g. `image.to_rgb8()`), keeping the trait boundary engine-agnostic.
///
/// # Example
///
/// ```rust
/// # #[cfg(feature = "ocr")]
/// # {
/// use image::DynamicImage;
/// use docuparse::ocr::{OcrBackend, OcrConfig, OcrPageResult};
///
/// struct AlwaysEmpty;
///
/// impl OcrBackend for AlwaysEmpty {
///     fn run(
///         &self,
///         _image: &DynamicImage,
///         page_index: u32,
///         _config: &OcrConfig,
///     ) -> Result<OcrPageResult, Box<dyn std::error::Error + Send + Sync>> {
///         Ok(OcrPageResult::builder()
///             .page_index(page_index)
///             .text(String::new())
///             .build())
///     }
/// }
/// # }
/// ```
pub trait OcrBackend: Send + Sync {
    fn run(
        &self,
        image: &DynamicImage,
        page_index: u32,
        config: &OcrConfig,
    ) -> Result<OcrPageResult, Box<dyn std::error::Error + Send + Sync>>;
}
