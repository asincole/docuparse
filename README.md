# docuparse

PDF processing for Rust - text extraction, page rendering, and pluggable OCR.

Backed by [Pdfium](https://pdfium.googlesource.com/pdfium/), the C++ PDF engine used by Google Chrome. Supports
pluggable OCR backends for scanned documents. The Rust layer adds negligible overhead - >99% of execution time is inside
pdfium and ONNX Runtime.

## Features

- Native text extraction from PDF text layers
- Page rendering to images at configurable DPI
- Automatic detection of scanned vs native-text pages
- PDF validation - magic bytes, size guards, path checks
- Document metadata - title, author, page count, file size, PDF version
- Pluggable OCR backends via `Arc<dyn OcrBackend>`
- Zero temp file I/O - everything in memory

## Setup

Docuparse links against Pdfium at runtime. Set `PDFIUM_LIB_PATH` to the directory containing the Pdfium shared library
before running:

```bash
export PDFIUM_LIB_PATH=/path/to/pdfium/lib
```

Pre-built Pdfium binaries for all platforms are available at
[github.com/bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries).
In this repo, `mise run pdfium` fetches the pinned build per platform and
syncs it into `pdfium-lib/`, and `mise run models` fetches the checksum-pinned
PP-OCR model pair used by the ONNX backend.

## CLI

`docuparse-cli` (in `crates/docuparse-cli`) exposes text extraction, page
rendering, and OCR-fallback extraction on the command line. Prebuilt binaries
for macOS (Intel and Apple Silicon), Linux (x86_64 and aarch64), and Windows
(x86_64) are attached to [releases](https://github.com/asincole/docuparse/releases)
under `cli-v*` tags, each with a `.sha256` checksum.

Install the binary via mise's HTTP backend:

```toml
[tools."http:docuparse-cli"]
version = "0.1.0"

[tools."http:docuparse-cli".platforms]
macos-arm64 = { url = "https://github.com/asincole/docuparse/releases/download/cli-v{{version}}/docuparse-cli-aarch64-apple-darwin.tar.gz", checksum = "sha256:<from SHA256SUMS>" }
macos-x64 = { url = "https://github.com/asincole/docuparse/releases/download/cli-v{{version}}/docuparse-cli-x86_64-apple-darwin.tar.gz", checksum = "sha256:<from SHA256SUMS>" }
linux-x64 = { url = "https://github.com/asincole/docuparse/releases/download/cli-v{{version}}/docuparse-cli-x86_64-unknown-linux-gnu.tar.gz", checksum = "sha256:<from SHA256SUMS>" }
linux-arm64 = { url = "https://github.com/asincole/docuparse/releases/download/cli-v{{version}}/docuparse-cli-aarch64-unknown-linux-gnu.tar.gz", checksum = "sha256:<from SHA256SUMS>" }
```

Once published to crates.io, `cargo install docuparse-cli` works as well; the
linux x86_64 build requires glibc >= 2.38 (the bundled ONNX Runtime's floor).

## Usage

### Text extraction

```rust,no_run
use docuparse::{PdfDocument, PdfOpenConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc = PdfDocument::load("document.pdf", &PdfOpenConfig::builder().build())?;

    println!("{} pages  {}", doc.page_count(), doc.metadata.file_size_display());

    for result in doc.extract_all_text_layers() {
        match result {
            Ok((page, Some(text))) => println!("page {}: {} chars", page + 1, text.len()),
            Ok((page, None)) => println!("page {}: scanned - no text layer", page + 1),
            Err(e) => eprintln!("error: {e}"),
        }
    }
    Ok(())
}
```

### Page rendering

```rust,no_run
use docuparse::{PdfDocument, PdfOpenConfig, RenderConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc = PdfDocument::load("document.pdf", &PdfOpenConfig::builder().build())?;

    let config = RenderConfig::builder().dpi(150).build();
    let image = doc.render_page(0, &config)?;
    image.save("page_1.png")?;

    // Render all pages
    let images = doc.render_all_pages(&config)?;
    for (i, image) in images.iter().enumerate() {
        image.save(format!("page_{:03}.png", i + 1))?;
    }
    Ok(())
}
```

## OCR

OCR is opt-in via feature flags. Two backends are provided:

The CLI can preserve native text and OCR only pages without a text layer:

```bash
cargo run -p docuparse-cli --features ocr -- text --ocr-fallback --json document.pdf
```

Set `PDFIUM_LIB_PATH`, `OCR_DET_MODEL_PATH`, `OCR_REC_MODEL_PATH`, and `OCR_DICT_PATH` before running. The flag is opt-in; native-text extraction errors still stop the command. The ONNX backend is initialized only when a selected page has no native text.

### ONNX (local inference)

Runs inference locally using ONNX Runtime. No network calls, no API keys.

```toml
[dependencies]
docuparse = { version = "*", features = ["ocr-onnx"] }
```

Requires three model artifacts set via environment variables:

```bash
export OCR_DET_MODEL_PATH=/path/to/det.onnx
export OCR_REC_MODEL_PATH=/path/to/rec.onnx
export OCR_DICT_PATH=/path/to/dict.txt
```

```rust,no_run
use docuparse::ocr::{OcrConfig, OnnxOcrBackend};
use docuparse::{OcrError, PdfDocument, PdfOpenConfig, RenderConfig};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = Arc::new(OnnxOcrBackend::from_env()?);

    let doc = PdfDocument::load(
        "scanned.pdf",
        &PdfOpenConfig::builder().ocr_backend(backend).build(),
    )?;

    let config = RenderConfig::builder().dpi(200).build();
    let ocr_config = OcrConfig::builder().confidence_threshold(0.5).build();

    let results = doc.ocr_all_pages(&config, &ocr_config)?;

    for page in &results {
        if page.has_text() {
            println!("page {}: {}", page.page_index + 1, page.text);
        }
    }
    Ok(())
}
```

Enable the `parallel` feature to run inference across rayon's thread pool:

```toml
docuparse = { version = "*", features = ["ocr-onnx", "parallel"] }
```

### OpenAI-compatible HTTP (vision API)

Sends rendered page images to any OpenAI-compatible vision endpoint - llama.cpp server, Ollama, vLLM, or the OpenAI API
itself.

```toml
[dependencies]
docuparse = { version = "*", features = ["ocr-openai"] }
```

```rust,no_run
use docuparse::ocr::{LlamaServerBackend, OcrConfig};
use docuparse::{PdfDocument, PdfOpenConfig, RenderConfig};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = Arc::new(LlamaServerBackend::new(
        "http://localhost:8080/v1/chat/completions",
    ));

    let doc = PdfDocument::load(
        "scanned.pdf",
        &PdfOpenConfig::builder().ocr_backend(backend).build(),
    )?;

    let results = doc.ocr_all_pages(
        &RenderConfig::builder().dpi(150).build(),
        &OcrConfig::builder().build(),
    )?;
    Ok(())
}
```

### Custom backends

Implement `OcrBackend` to plug in any inference engine:

```rust,no_run
use docuparse::ocr::{OcrBackend, OcrConfig, OcrPageResult};
use image::DynamicImage;

struct MyBackend;

impl OcrBackend for MyBackend {
    fn run(
        &self,
        image: &DynamicImage,
        page_index: u32,
        config: &OcrConfig,
    ) -> Result<OcrPageResult, Box<dyn std::error::Error + Send + Sync>> {
        let text = String::new(); // your inference here
        Ok(OcrPageResult::builder()
            .page_index(page_index)
            .text(text)
            .build())
    }
}
```

## Error handling

Each subsystem has its own error type. Import only what you use:

```rust,ignore
use docuparse::{LoadError, RenderError, TextError};
// with ocr feature:
use docuparse::OcrError;
```

```rust,no_run
use docuparse::{LoadError, PdfDocument, PdfOpenConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match PdfDocument::load("file.pdf", &PdfOpenConfig::builder().build()) {
        Err(LoadError::NotFound { path }) => eprintln!("not found: {}", path.display()),
        Err(LoadError::InvalidMagicBytes) => eprintln!("not a PDF"),
        Err(LoadError::PdfiumLibPathNotSet) => eprintln!("set PDFIUM_LIB_PATH"),
        Err(e) => eprintln!("error: {e}"),
        Ok(_doc) => {}
    }
    Ok(())
}
```

## Performance

Benchmarked on Apple Silicon (M4), release build, synthetic fixture PDFs:

| Operation                     | Pages | Time    | Per page |
|-------------------------------|-------|---------|----------|
| Text extraction               | 10    | ~3.4 ms | ~340 µs  |
| Page rendering (150 DPI)      | 5     | ~77 ms  | ~15 ms   |
| OCR - ONNX pipeline           | 10    | ~6.6 s  | ~660 ms  |
| OCR - ONNX parallel (chunk=8) | 10    | ~4.8 s  | ~480 ms  |

OCR throughput is dominated by model inference time. The `parallel` feature provides meaningful speedup on documents
with many pages where rayon can distribute inference across cores.

## License

MIT
