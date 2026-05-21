// Usage: cargo run --example ocr_openai --features ocr-openai [PDF_PATH] [ENDPOINT]
// Defaults:
//   PDF_PATH  examples/fixtures/sample_scanned.pdf
//   ENDPOINT  http://localhost:8080/v1/chat/completions

use std::{
    io::{BufWriter, Write},
    sync::Arc,
    time::Instant,
};

use docuparse::{
    PdfDocument, PdfOpenConfig, RenderConfig,
    ocr::{LlamaServerBackend, OcrConfig},
};

#[hotpath::main]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    let path = args
        .get(1)
        .map(String::as_str)
        .unwrap_or("examples/fixtures/sample_scanned.pdf");

    let endpoint = args
        .get(2)
        .map(String::as_str)
        .unwrap_or("http://localhost:8080/v1/chat/completions");

    let backend = Arc::new(LlamaServerBackend::new(endpoint));
    let config = PdfOpenConfig::builder().ocr_backend(backend).build();
    let doc = PdfDocument::load(path, &config)?;
    println!(
        "opened {} pages  {}",
        doc.page_count(),
        doc.metadata.file_size_display()
    );

    let render_cfg = RenderConfig::builder().dpi(150).build();
    let ocr_cfg = OcrConfig::builder().confidence_threshold(0.1).build();

    let t = Instant::now();
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
