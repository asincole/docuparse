// OCR a scanned PDF using a local ONNX model.
// Reads model paths from environment variables:
//   OCR_DET_MODEL_PATH, OCR_REC_MODEL_PATH, OCR_DICT_PATH
//
// Usage:
//   cargo run --example ocr_onnx --features ocr-onnx [PDF_PATH]

use std::{
    io::{BufWriter, Write},
    sync::Arc,
    time::Instant,
};

use docuparse::{
    PdfDocument, PdfOpenConfig, RenderConfig,
    ocr::{OcrConfig, OnnxOcrBackend},
};

#[hotpath::main]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "examples/fixtures/sample_scanned.pdf".to_string());

    let t = Instant::now();
    let backend = Arc::new(OnnxOcrBackend::from_env()?);
    println!("backend ready in {}ms", t.elapsed().as_millis());

    let config = PdfOpenConfig::builder().ocr_backend(backend).build();

    let doc = PdfDocument::load(&path, &config)?;
    println!(
        "opened {} pages  {}",
        doc.page_count(),
        doc.metadata.file_size_display()
    );

    let render_cfg = RenderConfig::builder().dpi(200).build();
    let ocr_cfg = OcrConfig::builder().confidence_threshold(0.1).build();

    let t = Instant::now();
    // ocr_pipeline bounds memory to 2 rendered images in flight at once.
    let results = doc.ocr_pipeline(&render_cfg, &ocr_cfg, 2)?;

    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    for page in &results {
        if page.has_text() {
            writeln!(
                out,
                "page {:>3}: {} chars",
                page.page_index + 1,
                page.text.len()
            )?;
        } else {
            writeln!(out, "page {:>3}: no text detected", page.page_index + 1)?;
        }
    }

    writeln!(out, "\n{}ms", t.elapsed().as_millis())?;

    Ok(())
}
