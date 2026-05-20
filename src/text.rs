use std::sync::Arc;

use pdfium_render::prelude::PdfPageIndex;

use crate::PdfDocument;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TextError {
    #[error("failed to extract text from page {page}")]
    ExtractionFailed {
        page: u32,
        #[source]
        source: Arc<dyn std::error::Error + Send + Sync>,
    },
}

#[cfg_attr(feature = "hotpath", hotpath::measure_all)]
impl PdfDocument {
    /// Extract the native text layer from a page.
    ///
    /// Returns `Ok(None)` if the page exists but has no text layer (e.g. scanned image).
    /// Returns `Err` if the page index is out of range or pdfium fails.
    pub fn extract_text_layer(&self, page_index: u32) -> Result<Option<String>, TextError> {
        let page = self
            .pages()
            .get(page_index as PdfPageIndex)
            .map_err(|err| TextError::ExtractionFailed {
                page: page_index,
                source: Arc::new(err),
            })?;

        let text_object = page.text().map_err(|err| TextError::ExtractionFailed {
            page: page_index,
            source: Arc::new(err),
        })?;

        let text = text_object.all();

        if text.trim().is_empty() {
            Ok(None)
        } else {
            Ok(Some(text))
        }
    }

    /// Returns a lazy iterator of `(page_index, Option<text>)` for every page.
    ///
    /// `Ok((i, Some(text)))` - page has a native text layer.
    /// `Ok((i, None))`       - page exists but has no text layer (scanned); needs OCR.
    /// `Err(_)`              - pdfium failed to access or extract from the page.
    pub fn extract_all_text_layers(
        &self,
    ) -> impl Iterator<Item = Result<(u32, Option<String>), TextError>> {
        (0..self.page_count()).map(|i| self.extract_text_layer(i).map(|text| (i, text)))
    }
}
