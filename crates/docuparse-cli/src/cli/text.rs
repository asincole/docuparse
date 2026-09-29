use docuparse::{PdfDocument, PdfOpenConfig};
#[cfg(feature = "ocr")]
use docuparse::{
    RenderConfig,
    ocr::{OcrBackend, OcrConfig, OnnxOcrBackend},
};
use usage_rs::{Args, Run};

use super::{error::fail, pages::PageRange};

/// Extract text from a PDF's native text layer, page by page.
#[derive(Args)]
pub(crate) struct Text {
    /// Path to the PDF file
    path: String,

    /// Pages to extract, e.g. "1,3,5-8" (1-based). Defaults to all pages.
    #[usage(long)]
    pages: Option<PageRange>,

    /// Print machine-readable JSON instead of plain text
    #[usage(long)]
    json: bool,

    /// OCR pages with no native text using the local ONNX models
    #[cfg(feature = "ocr")]
    #[usage(long)]
    ocr_fallback: bool,
}

impl Run for Text {
    type Output = ();

    fn run(self) {
        let doc = PdfDocument::load(&self.path, &PdfOpenConfig::builder().build())
            .unwrap_or_else(|e| fail(e));

        let wanted = match &self.pages {
            Some(range) => range.resolve(doc.page_count()).unwrap_or_else(|e| fail(e)),
            None => PageRange::all(doc.page_count()),
        };

        #[cfg(feature = "ocr")]
        let mut ocr_backend = None;
        #[cfg(feature = "ocr")]
        let render_config = RenderConfig::builder().dpi(200).build();
        #[cfg(feature = "ocr")]
        let ocr_config = OcrConfig::builder().confidence_threshold(0.5).build();

        let mut pages_out = Vec::new();

        for page in wanted {
            let text = doc.extract_text_layer(page).unwrap_or_else(|e| fail(e));
            #[cfg(feature = "ocr")]
            let text = if self.ocr_fallback && text.is_none() {
                let backend = ocr_backend
                    .get_or_insert_with(|| OnnxOcrBackend::from_env().unwrap_or_else(|e| fail(e)));
                let image = doc
                    .render_page(page, &render_config)
                    .unwrap_or_else(|e| fail(e));
                let result = backend
                    .run(&image, page, &ocr_config)
                    .unwrap_or_else(|e| fail(e));
                (!result.text.trim().is_empty()).then_some(result.text)
            } else {
                text
            };

            if self.json {
                pages_out.push(serde_json::json!({ "page": page + 1, "text": text }));
            } else {
                match &text {
                    Some(t) => println!("=== page {} ===\n{t}\n", page + 1),
                    None => println!("=== page {} (scanned - no text layer) ===\n", page + 1),
                }
            }
        }

        if self.json {
            println!("{}", serde_json::json!({ "pages": pages_out }));
        }
    }
}
