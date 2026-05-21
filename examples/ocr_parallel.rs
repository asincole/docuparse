// Renders pages sequentially (PDFium is not thread-safe) then runs OCR
// inference in parallel across rayon's thread pool.
//
// Usage: cargo run --example ocr_parallel --features ocr-onnx,parallel [PDF_PATH]
// Parallel inference pays off on documents with many pages.
// On small documents the rayon overhead may exceed the speedup.

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
    // With the `parallel` feature, ocr_all_pages renders sequentially then
    // dispatches inference across rayon workers in bounded chunks.
    let results = doc.ocr_all_pages(&render_cfg, &ocr_cfg)?;

    let mut out = BufWriter::new(std::io::stdout().lock());
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
