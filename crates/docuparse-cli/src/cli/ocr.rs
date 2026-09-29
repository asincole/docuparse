use std::{collections::HashSet, sync::Arc};

use docuparse::{
    PdfDocument, PdfOpenConfig, RenderConfig,
    ocr::{LlamaServerBackend, OcrBackend, OcrConfig, OnnxOcrBackend},
};
use usage_rs::{Args, Run};

use super::{error::fail, pages::PageRange};

/// Run OCR over PDF pages using a local ONNX model or an OpenAI-compatible vision endpoint.
#[derive(Args)]
pub(crate) struct Ocr {
    /// Path to the PDF file
    path: String,

    /// Which OCR backend to use
    #[usage(long, choices("onnx", "openai"))]
    backend: String,

    /// OpenAI-compatible chat completions endpoint (backend=openai only)
    #[usage(long, default = "http://localhost:8080/v1/chat/completions")]
    endpoint: String,

    /// Pages to OCR, e.g. "1,3,5-8" (1-based). Defaults to all pages.
    #[usage(long)]
    pages: Option<PageRange>,

    /// Render resolution in DPI before OCR
    #[usage(long, default = "200")]
    dpi: u32,

    /// Minimum confidence for detected text (0.0-1.0)
    #[usage(long, default = "0.5")]
    confidence: f32,

    /// Max rendered pages held in memory at once (backend=onnx only)
    #[usage(long, default = "2")]
    chunk_size: usize,

    /// Print machine-readable JSON instead of a summary
    #[usage(long)]
    json: bool,
}

impl Run for Ocr {
    type Output = ();

    fn run(self) {
        let backend: Arc<dyn OcrBackend> = match self.backend.as_str() {
            "onnx" => Arc::new(OnnxOcrBackend::from_env().unwrap_or_else(|e| fail(e))),
            _ => Arc::new(LlamaServerBackend::new(&self.endpoint)),
        };

        let config = PdfOpenConfig::builder().ocr_backend(backend).build();
        let doc = PdfDocument::load(&self.path, &config).unwrap_or_else(|e| fail(e));

        let wanted: HashSet<u32> = match &self.pages {
            Some(range) => range.resolve(doc.page_count()).unwrap_or_else(|e| fail(e)),
            None => PageRange::all(doc.page_count()),
        }
        .into_iter()
        .collect();

        let render_cfg = RenderConfig::builder().dpi(self.dpi).build();
        let ocr_cfg = OcrConfig::builder()
            .confidence_threshold(self.confidence)
            .build();

        // Both `ocr_pipeline` and `ocr_all_pages` process every page - docuparse
        // doesn't expose a page-subset entry point, so `--pages` filters results
        // after the fact rather than skipping unwanted pages during inference.
        let results = if self.backend == "onnx" {
            doc.ocr_pipeline(&render_cfg, &ocr_cfg, self.chunk_size)
        } else {
            doc.ocr_all_pages(&render_cfg, &ocr_cfg)
        }
        .unwrap_or_else(|e| fail(e));

        let mut pages_out = Vec::new();

        for page in &results {
            if !wanted.contains(&page.page_index) {
                continue;
            }

            if self.json {
                pages_out.push(serde_json::json!({
                    "page": page.page_index + 1,
                    "text": page.text,
                }));
            } else if page.has_text() {
                println!("=== page {} ===\n{}\n", page.page_index + 1, page.text);
            } else {
                println!("=== page {} (no text detected) ===\n", page.page_index + 1);
            }
        }

        if self.json {
            println!("{}", serde_json::json!({ "pages": pages_out }));
        }
    }
}
