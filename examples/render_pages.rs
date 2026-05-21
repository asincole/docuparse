// Usage: cargo run --example render_pages [PDF_PATH] [OUT_DIR] [DPI]

use std::{path::PathBuf, time::Instant};

use docuparse::{PdfDocument, PdfOpenConfig, RenderConfig};

#[hotpath::main]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    let pdf_path = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("examples/fixtures/sample_mixed.pdf"));

    let out_dir = args
        .get(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("examples/fixtures/rendered"));

    let dpi: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(150);

    std::fs::create_dir_all(&out_dir)?;

    let doc = PdfDocument::load(&pdf_path, &PdfOpenConfig::builder().build())?;
    println!(
        "opened {} pages  {}  dpi={dpi}",
        doc.page_count(),
        doc.metadata.file_size_display()
    );

    let config = RenderConfig::builder().dpi(dpi).build();
    let t = Instant::now();
    let mut rendered = 0usize;
    let mut skipped = 0usize;

    for page_index in 0..doc.page_count() {
        match doc.render_page(page_index, &config) {
            Ok(image) => {
                let out_path = out_dir.join(format!("page_{:03}.png", page_index + 1));
                image.save(&out_path)?;
                println!(
                    "  page {:>3}  {}×{}px",
                    page_index + 1,
                    image.width(),
                    image.height()
                );
                rendered += 1;
            }
            Err(e) => {
                eprintln!("  page {:>3}  error: {e}", page_index + 1);
                skipped += 1;
            }
        }
    }

    println!(
        "{rendered} rendered  {skipped} skipped  {}ms",
        t.elapsed().as_millis()
    );

    Ok(())
}
