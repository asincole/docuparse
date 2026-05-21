// Measures text extraction throughput over 5 runs.
//
// Usage:
//   cargo run --example benchmark [PDF_PATH]

use std::time::Instant;

use docuparse::{PdfDocument, PdfOpenConfig};

#[hotpath::main]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "examples/fixtures/sample_mixed.pdf".to_string());

    // cold run - JIT, page cache warm-up, not reported
    let _ = run(&path);

    println!("path: {path}");

    let mut timings = Vec::new();

    for i in 1..=5 {
        let (us, pages, chars) = run(&path)?;
        timings.push(us);
        println!("  run {i}: {:<10}  {pages} pages  {chars} chars", fmt(us));
    }

    let avg = timings.iter().sum::<u128>() / timings.len() as u128;
    println!(
        "  avg: {}  min: {}  max: {}",
        fmt(avg),
        fmt(*timings.iter().min().unwrap()),
        fmt(*timings.iter().max().unwrap()),
    );

    Ok(())
}

fn run(path: &str) -> Result<(u128, usize, usize), Box<dyn std::error::Error>> {
    let config = PdfOpenConfig::builder().build();
    let t = Instant::now();
    let doc = PdfDocument::load(path, &config)?;

    let mut chars = 0usize;
    let mut pages = 0usize;

    for result in doc.extract_all_text_layers() {
        pages += 1;
        if let Ok((_, Some(text))) = result {
            chars += text.len();
        }
    }

    Ok((t.elapsed().as_micros(), pages, chars))
}

fn fmt(us: u128) -> String {
    if us >= 1_000_000 {
        format!("{:.2}s", us as f64 / 1_000_000.0)
    } else if us >= 1_000 {
        format!("{:.2}ms", us as f64 / 1_000.0)
    } else {
        format!("{us}µs")
    }
}
