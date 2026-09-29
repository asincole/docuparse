# docuparse-cli

Command-line PDF text extraction, rendering, and OCR built on
[docuparse](https://github.com/asincole/docuparse).

```sh
# native text layer per page
docuparse-cli text --json input.pdf

# OCR pages that have no usable native text (PP-OCR models via env)
docuparse-cli text --ocr-fallback --json input.pdf
```

OCR uses `PDFIUM_LIB_PATH` (directory containing the pdfium library) and
`OCR_DET_MODEL_PATH` / `OCR_REC_MODEL_PATH` / `OCR_DICT_PATH`. Prebuilt
binaries for macOS, Linux, and Windows are attached to
[releases](https://github.com/asincole/docuparse/releases) under the
`cli-v*` tags.
