# docuparse-cli

Command-line PDF text extraction, metadata inspection, page rendering, and OCR
built on [docuparse](https://github.com/asincole/docuparse).

## Installation and setup

Install the core CLI with Cargo, or enable OCR with the `ocr` feature:

```text
cargo install docuparse-cli
cargo install docuparse-cli --features ocr
```

Binary release assets use `cli-v*` tags on the project's
[releases page](https://github.com/asincole/docuparse/releases).

Every PDF command needs the Pdfium shared library. Set `PDFIUM_LIB_PATH` to the
directory containing `libpdfium.dylib`, `libpdfium.so`, or `pdfium.dll` for your
platform. Local ONNX OCR also needs compatible detection and recognition models
and their character dictionary:

| Environment variable | Purpose |
| --- | --- |
| `PDFIUM_LIB_PATH` | Directory containing the Pdfium shared library |
| `OCR_DET_MODEL_PATH` | Detection model `.onnx` file |
| `OCR_REC_MODEL_PATH` | Recognition model `.onnx` file |
| `OCR_DICT_PATH` | Character dictionary matching the recognition model |

For a source checkout, `mise run bootstrap` installs the pinned Pdfium library
and OCR models. Set their paths in a gitignored `mise.local.toml`:

```toml
[env]
PDFIUM_LIB_PATH = "./pdfium-lib"
OCR_DET_MODEL_PATH = "./models/ppocrv6-small/pp-ocrv6_small_det.onnx"
OCR_REC_MODEL_PATH = "./models/ppocrv6-small/pp-ocrv6_small_rec.onnx"
OCR_DICT_PATH = "./models/ppocrv6-small/ppocrv6_dict.txt"
```

Build the OCR-enabled CLI with `mise run build-cli`. Run the resulting binary
through `mise exec -- target/release/docuparse-cli` so it receives the configured
environment. Endpoint OCR needs a running server that accepts image inputs at
the full `/v1/chat/completions` URL; it does not need local ONNX model paths.
The CLI does not expose authentication, model selection, or prompt configuration
flags for endpoint requests.

## Commands and page selection

```text
docuparse-cli --help
docuparse-cli --version
docuparse-cli info --help
docuparse-cli text --help
docuparse-cli render --help
docuparse-cli ocr --help
docuparse-cli completion --help
```

`info`, `text`, `render`, and `completion` are available in the core build.
The `ocr` command and `text --ocr-fallback` / `--endpoint` require the `ocr`
feature. All commands support `--help`.

`text`, `render`, and `ocr` accept `--pages`, for example `--pages 1,3,5-8`.
Page numbers are one-based, ranges include both ends, and omitted selections
process all pages. Selections are sorted and deduplicated; output retains the
original PDF page numbers. Zero, reversed ranges, and pages beyond the document
are errors.

## Inspect document metadata

```text
docuparse-cli info document.pdf
docuparse-cli info --json document.pdf
```

The default summary shows the document name, page count, file size, title, author,
subject, creator, producer, and PDF version. Missing optional text values appear
as `-` in the summary and `null` in JSON.

Example JSON:

```json
{
  "pdf_name": "document",
  "page_count": 8,
  "file_size_bytes": 245760,
  "title": "Example document",
  "author": null,
  "subject": null,
  "creator": null,
  "producer": null,
  "pdf_version": "Pdf1_7"
}
```

`pdf_name` is the file stem without its extension. The existing `info` output
uses version names such as `Pdf1_7`; extraction metadata uses strings such as
`1.7`.

## Extract text

```text
docuparse-cli text document.pdf
docuparse-cli text --pages 1-2 --json document.pdf
docuparse-cli text --ocr-fallback --json document.pdf
docuparse-cli text --endpoint http://localhost:8080/v1/chat/completions --json document.pdf
```

Plain-text output separates pages with `=== page 1 ===`. Pages without usable
text have a marker such as `=== page 2 (no text extracted) ===`.

| Option | Behavior |
| --- | --- |
| `--pages <PAGES>` | Select pages; defaults to all pages |
| `--json` | Emit page records as JSON |
| `--metadata` | Include document metadata and provenance; requires `--json` |
| `--ocr-fallback` | Use local ONNX OCR on pages with no native text |
| `--endpoint <URL>` | Use a vision endpoint instead of ONNX; implies fallback |

Without `--metadata`, JSON has this format, including `null` for empty pages:

```json
{"pages":[{"page":1,"text":"Example text"},{"page":2,"text":null}]}
```

Fallback runs only after successful native extraction returns no text. Native
extraction errors fail the command, as do OCR errors. The backend initializes
only when a page needs fallback, and fallback processes one page at a time.
Text fallback uses 200 DPI and an ONNX confidence threshold of 0.5; use the `ocr`
command to configure these values for explicit OCR runs.

### Include metadata and provenance

```text
docuparse-cli text --pages 1-2 --ocr-fallback --json --metadata document.pdf
```

Example output:

```json
{
  "document": {
    "file_name": "document.pdf",
    "page_count": 8,
    "file_size_bytes": 245760,
    "pdf_version": "1.7",
    "title": null,
    "author": null,
    "subject": null,
    "creator": null,
    "producer": null
  },
  "extraction": {
    "extracted_page_count": 2,
    "native_pages": 1,
    "ocr_pages": 1,
    "empty_pages": 0,
    "ocr": {
      "fallback_enabled": true,
      "backend": "onnx",
      "render_dpi": 200,
      "confidence_threshold": 0.5
    }
  },
  "pages": [
    {"page": 1, "text": "Native text...", "source": "native", "fallback_reason": null},
    {"page": 2, "text": "Recognized text...", "source": "ocr", "fallback_reason": "no_native_text"}
  ]
}
```

`document.page_count` describes the whole PDF, regardless of `--pages`.
`file_name` is the basename including its extension, without the full input
path. Missing optional document values are `null`.

| Extraction field | Meaning |
| --- | --- |
| `extracted_page_count` | Returned page records, including empty pages |
| `native_pages` | Pages processed without OCR fallback |
| `ocr_pages` | Pages where OCR fallback was attempted, including empty results |
| `empty_pages` | Pages whose final text is `null`; overlaps both method counts |
| `ocr` | Effective fallback configuration, or `null` when disabled |

`native_pages + ocr_pages` equals `extracted_page_count`, and `empty_pages` cannot
exceed it. Empty pages still have a `source`: `native` without fallback, or `ocr`
after fallback. `fallback_reason` is `no_native_text` when OCR was attempted and
`null` otherwise.

When fallback is enabled, `extraction.ocr` describes its configuration even if
every selected page has native text. Local fallback reports `backend: "onnx"`;
`--endpoint` reports `backend: "ocr_endpoint"`. The confidence threshold describes
the configured filter, not measured accuracy; endpoint OCR does not apply the
ONNX confidence filter. Metadata excludes endpoint URLs, credentials, and model
paths. Enabling metadata preserves extraction behavior and the existing
metadata-disabled JSON and plain-text output.

## Render pages to PNG

```text
docuparse-cli render --out-dir rendered document.pdf
docuparse-cli render --pages 1-2 --dpi 200 --out-dir rendered --json document.pdf
docuparse-cli render --pages 1 --stdout document.pdf
```

| Option | Behavior |
| --- | --- |
| `--out-dir <DIR>` | Output directory, created if missing; required unless using `--stdout` |
| `--pages <PAGES>` | Select pages; defaults to all pages |
| `--dpi <DPI>` | Render resolution; defaults to 150 |
| `--json` | Emit a manifest of written PNG files |
| `--stdout` | Write raw PNG bytes for exactly one selected page |

Files use names such as `page_001.png` and `page_002.png`, based on their original
page numbers. Existing files at those paths are overwritten. The default summary
lists each written page and path; JSON uses this format:

```json
{"written":[{"page":1,"path":"rendered/page_001.png"},{"page":2,"path":"rendered/page_002.png"}]}
```

Manifest paths use the supplied output directory and the platform's path
separator. `--stdout` cannot be combined with `--json`; it requires a single page
and emits PNG bytes, with no text summary. In Nushell, save the binary output with:

```nu
docuparse-cli render --pages 1 --stdout document.pdf | save --raw --force page.png
```

## Run OCR explicitly

Requires a build with the `ocr` feature. This command runs OCR on every selected
page, including pages that already have native text.

```text
docuparse-cli ocr --backend onnx --json document.pdf
docuparse-cli ocr --backend onnx --pages 1-2 --dpi 300 --confidence 0.7 --json document.pdf
docuparse-cli ocr --backend onnx --chunk-size 4 document.pdf
docuparse-cli ocr --backend openai --endpoint http://localhost:8080/v1/chat/completions --json document.pdf
```

| Option | Behavior |
| --- | --- |
| `--backend <BACKEND>` | Required: `onnx` or `openai` |
| `--endpoint <URL>` | Endpoint for `openai`; defaults to `http://localhost:8080/v1/chat/completions` |
| `--pages <PAGES>` | Select pages; defaults to all pages |
| `--dpi <DPI>` | Render resolution before OCR; defaults to 200 |
| `--confidence <VALUE>` | ONNX confidence filter between 0.0 and 1.0; defaults to 0.5 |
| `--chunk-size <COUNT>` | Rendered pages held at once for full-document ONNX runs; defaults to 2 |
| `--json` | Emit recognized text as JSON |

`--chunk-size` applies only to full-document ONNX runs without `--pages`.
Selected-page runs process one page at a time. The endpoint backend does not use
the ONNX confidence filter. The command's backend argument is `openai`, while
the `text --metadata` endpoint label is `ocr_endpoint`.

Plain-text output separates recognized pages with page markers and uses
`=== page 2 (no text detected) ===` for empty results. JSON keeps empty OCR text
as an empty string:

```json
{"pages":[{"page":1,"text":"Recognized text"},{"page":2,"text":""}]}
```

`--metadata` is available only on `text`.

## Shell completion

Generate a completion script for Bash, Zsh, or Fish:

```text
docuparse-cli completion --shell bash
docuparse-cli completion --shell zsh
docuparse-cli completion --shell fish
```

`--shell` is required. The script is printed to stdout so you can save it and
load it using your shell's completion setup. For example, from Nushell:

```nu
docuparse-cli completion --shell fish | save --force docuparse-cli.fish
```

## Output and errors

Diagnostics go to stderr and failures exit with a nonzero status. JSON from
`text` and `ocr` is emitted only after extraction succeeds. Plain-text extraction
can produce earlier pages before a later page fails. Rendering writes files as
it processes pages, so files from earlier pages can remain after a failure.

Builds with profiling features can additionally emit hotpath reports. When using
such builds in pipelines, set `HOTPATH_OUTPUT_PATH` to a separate report file to
keep stdout reserved for command output.

## Development

Run these checks from the repository root:

```text
mise run fmt-check
mise run lint
mise run test
```

The repository tasks currently target the root library. To check the CLI too:

```text
mise exec -- cargo clippy -p docuparse-cli --all-features --all-targets -- -D warnings
mise exec -- cargo nextest run -p docuparse-cli --all-features
```
