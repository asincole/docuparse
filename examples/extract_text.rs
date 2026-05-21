use std::{
    io::{BufWriter, Write},
    time::Instant,
};

use docuparse::{PdfDocument, PdfOpenConfig};

#[hotpath::main]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "examples/fixtures/sample_native.pdf".to_string());

    let doc = PdfDocument::load(&path, &PdfOpenConfig::builder().build())?;

    println!(
        "opened {} pages  {}",
        doc.page_count(),
        doc.metadata.file_size_display(),
    );

    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    let t = Instant::now();

    for result in doc.extract_all_text_layers() {
        match result {
            Ok((page, Some(text))) => writeln!(out, "page {:>3}: {} chars", page + 1, text.len())?,
            Ok((page, None)) => writeln!(out, "page {:>3}: no text layer", page + 1)?,
            Err(e) => writeln!(out, "error: {e}")?,
        }
    }

    writeln!(out, "\n{}ms", t.elapsed().as_millis())?;

    Ok(())
}
