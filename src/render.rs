use image::DynamicImage;
use pdfium_render::prelude::{PdfPageIndex, PdfRenderConfig};

use crate::{
    error::{PdfError, RenderError},
    pdf_document::PdfDocument,
};

#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
pub struct RenderConfig {
    #[builder(default = 150)]
    pub dpi: u32,

    #[builder(default = false)]
    pub with_annotations: bool,

    #[builder(default = 4096)]
    pub max_dimension_px: i32,
}

impl<'p> PdfDocument<'p> {
    pub fn render_page(
        &self,
        page_index: u32,
        config: &RenderConfig,
    ) -> Result<DynamicImage, PdfError> {
        let page = self
            .pages()
            .get(page_index as PdfPageIndex)
            .map_err(|source| match source {
                pdfium_render::prelude::PdfiumError::PageIndexOutOfBounds => {
                    RenderError::PageOutOfRange {
                        page: page_index,
                        total: self.page_count(),
                    }
                }
                _ => RenderError::RenderFailed {
                    page: page_index,
                    source,
                },
            })?;

        // PdfPoints are 1/72 inch — multiply by dpi/72 to get target pixel dimensions.
        // All geometry stays in f32 until the single final cast to i32.
        let scale = config.dpi as f32 / 72.0;
        let width_f = page.width().value * scale;
        let height_f = page.height().value * scale;

        if width_f <= 0.0
            || height_f <= 0.0
            || width_f > i32::MAX as f32
            || height_f > i32::MAX as f32
        {
            return Err(RenderError::DegenerateGeometry {
                page: page_index,
                width_f,
                height_f,
            }
            .into());
        }

        // Clamp to max_dimension_px while staying in f32 to avoid
        // precision loss from repeated i32 <-> f32 casts.
        let (width_f, height_f) = if width_f > config.max_dimension_px as f32
            || height_f > config.max_dimension_px as f32
        {
            let factor = config.max_dimension_px as f32 / width_f.max(height_f);
            (width_f * factor, height_f * factor)
        } else {
            (width_f, height_f)
        };

        // Single cast to i32 after all float arithmetic is complete.
        let width = width_f as i32;
        let height = height_f as i32;

        let mut render_cfg = PdfRenderConfig::new()
            .set_target_width(width)
            .set_maximum_height(height);

        if config.with_annotations {
            render_cfg = render_cfg.render_annotations(true);
        }

        // NOTE: as_image() performs a full pixel-buffer copy out of pdfium's
        // internal bitmap. At high DPI this is a significant allocation.
        let image = page
            .render_with_config(&render_cfg)
            .map_err(|source| RenderError::RenderFailed {
                page: page_index,
                source,
            })?
            .as_image()
            .map_err(|source| RenderError::BitmapConversionFailed {
                page: page_index,
                source,
            })?;

        Ok(image)
    }

    /// Render all pages. Fails fast on the first page that cannot be rendered.
    pub fn render_all_pages(&self, config: &RenderConfig) -> Result<Vec<DynamicImage>, PdfError> {
        (0..self.page_count())
            .map(|i| self.render_page(i, config))
            .collect()
    }
}
