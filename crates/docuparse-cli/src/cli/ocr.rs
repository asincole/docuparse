use std::{
    error::Error,
    io::{self, BufWriter, Write},
    sync::Arc,
};

use docuparse::{
    PdfDocument, PdfOpenConfig, RenderConfig,
    ocr::{LlamaServerBackend, OcrBackend, OcrConfig, OnnxOcrBackend},
};
use serde::Serialize;
use usage_rs::{Args, Run};

use super::{error::fail, pages::PageRange};

/// Run OCR over PDF pages using local ONNX models or a vision endpoint.
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

    /// Max rendered pages held in memory at once.
    /// Applies to ONNX full-document runs only.
    #[usage(long, default = "2")]
    chunk_size: usize,

    /// Print machine-readable JSON instead of a summary
    #[usage(long)]
    json: bool,
}

#[derive(Serialize)]
struct OcrPage<'a> {
    page: u32,
    text: &'a str,
}

impl Run for Ocr {
    type Output = ();

    fn run(self) {
        self.execute().unwrap_or_else(|e| fail(e));
    }
}

impl Ocr {
    fn execute(self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let backend: Arc<dyn OcrBackend> = match self.backend.as_str() {
            "onnx" => Arc::new(OnnxOcrBackend::from_env()?),
            "openai" => Arc::new(LlamaServerBackend::new(&self.endpoint)),
            other => {
                return Err(format!("unsupported OCR backend: {other}").into());
            }
        };

        let config = PdfOpenConfig::builder()
            .ocr_backend(Arc::clone(&backend))
            .build();
        let doc = PdfDocument::load(&self.path, &config)?;

        let render_config = RenderConfig::builder().dpi(self.dpi).build();
        let ocr_config = OcrConfig::builder()
            .confidence_threshold(self.confidence)
            .build();

        // Full-document entry points cannot restrict processing to selected pages.
        let results = match &self.pages {
            Some(range) => {
                let wanted = range.resolve(doc.page_count())?;

                wanted
                    .into_iter()
                    .map(|page| {
                        let image = doc.render_page(page, &render_config)?;
                        backend.run(&image, page, &ocr_config)
                    })
                    .collect::<Result<Vec<_>, Box<dyn Error + Send + Sync>>>()?
            }
            None if self.backend == "onnx" => {
                doc.ocr_pipeline(&render_config, &ocr_config, self.chunk_size)?
            }
            None => doc.ocr_all_pages(&render_config, &ocr_config)?,
        };

        let stdout = io::stdout();

        if self.json {
            let mut json_output = Vec::new();
            json_output.extend_from_slice(b"{\"pages\":[");

            for (index, page) in results.into_iter().enumerate() {
                if index != 0 {
                    json_output.push(b',');
                }

                serde_json::to_writer(
                    &mut json_output,
                    &OcrPage {
                        page: page.page_index + 1,
                        text: &page.text,
                    },
                )?;
            }

            json_output.extend_from_slice(b"]}\n");

            let mut output = stdout.lock();
            output.write_all(&json_output)?;
            output.flush()?;

            return Ok(());
        }

        let mut output = BufWriter::new(stdout.lock());

        for page in results {
            let page_number = page.page_index + 1;

            if page.has_text() {
                writeln!(output, "=== page {page_number} ===\n{}\n", page.text,)?;
            } else {
                writeln!(output, "=== page {page_number} (no text detected) ===\n",)?;
            }
        }

        output.flush()?;
        Ok(())
    }
}
