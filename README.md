# docuparse

High-performance document extraction library for Rust.

Extracts text and renders pages from PDF documents using the
[Pdfium](https://pdfium.googlesource.com/pdfium/) engine — the same C++ library
used by Google Chrome.

## Features

- **Native text extraction** — extracts text directly from PDF text layers,
  no OCR required for digitally generated documents
- **Page rendering** — renders PDF pages to images at configurable DPI with
  optional annotation rendering and dimension clamping
- **Automatic page classification** — detects whether a page has a native
  text layer or is scanned
- **PDF validation** — magic byte checking, size guards, existence checks
- **Document metadata** — title, author, page count, file size, PDF version
- **Zero intermediate files** — everything in memory, no temp file I/O
- **Pdfium singleton** — initialised once, borrowed across your application

## Installation

```toml
[dependencies]
docuparse = "0.0.1"
```

Docuparse links against Pdfium at runtime. You must provide the Pdfium
shared library alongside your binary. Pre-built binaries are available from
the [pdfium-render releases](https://github.com/ajrcarey/pdfium-render).

## Quick Start

```rust
use docuparse::{init_pdfium, PdfDocument};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pdfium = init_pdfium()?;
    let doc = PdfDocument::open(pdfium, "document.pdf")?;

    println!("pages: {}", doc.page_count());
    println!("file:  {}", doc.metadata.file_size_display());

    for result in doc.extract_all_text_layers() {
        match result {
            Ok((page, Some(text))) => println!("page {}: {} chars", page + 1, text.len()),
            Ok((page, None))       => println!("page {}: scanned — no text layer", page + 1),
            Err(e)                 => eprintln!("page: error — {e}"),
        }
    }

    Ok(())
}
```

## Page Rendering

```rust
use docuparse::{init_pdfium, PdfDocument, RenderConfig};

let pdfium = init_pdfium()?;
let doc = PdfDocument::open(pdfium, "document.pdf")?;

let config = RenderConfig::builder()
    .dpi(150)
    .with_annotations(true)
    .build();

let image = doc.render_page(0, &config)?;
image.save("page_1.png")?;
```

### Batch Rendering

```rust
let images = doc.render_all_pages(&config)?;
for (i, image) in images.iter().enumerate() {
    image.save(format!("page_{:03}.png", i + 1))?;
}
```

## Performance

Benchmarked on a representative mixed PDF document (native text layers +
scanned pages) on Apple Silicon (M4):

| Metric | Value |
|---|---|
| Text extraction (12 pages) | ~8 ms |
| Per page | ~680 µs |
| Memory RSS | ~25 MB |
| Child processes spawned | 0 |
| Temp file I/O | none |

Performance is dominated by the Pdfium C++ engine. The Rust wrapper
contributes negligible overhead — dev and release builds measure
identically, confirming zero Rust-level bottleneck.

## Error Handling

All public functions return `Result<T, PdfError>`. Errors are fully typed
via [`thiserror`](https://github.com/dtolnay/thiserror) and compose cleanly
with `anyhow` or `eyre` in application code:

```rust
use docuparse::PdfError;

match PdfDocument::open(pdfium, "file.pdf") {
    Err(PdfError::Validation(_)) => {
        eprintln!("not a valid PDF");
    }
    Err(e) => eprintln!("error: {e}"),
    Ok(doc) => { /* ... */ }
}
```

## Roadmap

- [ ] OCR support via ONNX Runtime (feature flag)
- [ ] Image input (JPEG, PNG) alongside PDF
- [ ] Async API

## License

MIT
