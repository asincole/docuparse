use std::{
    error::Error,
    io::{self, BufWriter, Write},
    path::Path,
};

use docuparse::{PdfDocument, PdfOpenConfig};
#[cfg(feature = "ocr")]
use docuparse::{
    RenderConfig,
    ocr::{LlamaServerBackend, OcrBackend, OcrConfig, OnnxOcrBackend},
};
use serde::Serialize;
use usage_rs::{Args, Run};

use super::{document_info::DocumentInfo, error::fail, pages::PageRange};

type ExtractionError = Box<dyn Error + Send + Sync>;

/// Extract text page by page, optionally using OCR for missing text layers.
#[derive(Args)]
pub(crate) struct Text {
    /// Path to the PDF file
    path: String,

    /// Pages to extract, e.g. "1,3,5-8" (1-based). Defaults to all pages.
    #[usage(long)]
    pages: Option<PageRange>,

    /// Print machine-readable JSON instead of plain text
    #[usage(long)]
    json: bool,

    /// Include document metadata and extraction provenance (requires --json)
    #[usage(long, requires = "--json")]
    metadata: bool,

    /// OCR pages with no native text, using the local ONNX models
    #[cfg(feature = "ocr")]
    #[usage(long)]
    ocr_fallback: bool,

    /// OpenAI-compatible vision endpoint for OCR fallback instead of
    /// local models; implies --ocr-fallback
    #[cfg(feature = "ocr")]
    #[usage(long)]
    endpoint: Option<String>,
}

#[derive(Serialize)]
struct PageText<'a> {
    page: u32,
    text: Option<&'a str>,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum TextSource {
    Native,
    Ocr,
}

#[derive(Serialize)]
struct MetadataPage<'a> {
    #[serde(flatten)]
    page: PageText<'a>,
    source: TextSource,
    fallback_reason: Option<&'static str>,
}

#[derive(Serialize)]
struct TextDocument<'a> {
    file_name: &'a str,
    #[serde(flatten)]
    info: DocumentInfo<'a>,
}

#[derive(Clone, Copy, Serialize)]
struct OcrInfo {
    fallback_enabled: bool,
    backend: &'static str,
    render_dpi: u32,
    confidence_threshold: f32,
}

#[derive(Default, Serialize)]
struct ExtractionInfo {
    extracted_page_count: usize,
    native_pages: usize,
    ocr_pages: usize,
    empty_pages: usize,
    ocr: Option<OcrInfo>,
}

impl Run for Text {
    type Output = ();

    fn run(self) {
        self.execute().unwrap_or_else(|e| fail(e));
    }
}

impl Text {
    fn execute(self) -> Result<(), ExtractionError> {
        if self.metadata && !self.json {
            return Err("--metadata requires --json".into());
        }

        let doc = PdfDocument::load(&self.path, &PdfOpenConfig::builder().build())?;

        let wanted = match &self.pages {
            Some(range) => range.resolve(doc.page_count())?,
            None => PageRange::all(doc.page_count()),
        };

        #[cfg(feature = "ocr")]
        let fallback = self.ocr_fallback || self.endpoint.is_some();
        #[cfg(feature = "ocr")]
        let mut ocr_backend: Option<Box<dyn OcrBackend>> = None;
        #[cfg(feature = "ocr")]
        let render_config = RenderConfig::builder().dpi(200).build();
        #[cfg(feature = "ocr")]
        let ocr_config = OcrConfig::builder().confidence_threshold(0.5).build();

        #[cfg(feature = "ocr")]
        let ocr_info = fallback.then_some(OcrInfo {
            fallback_enabled: true,
            backend: if self.endpoint.is_some() {
                "ocr_endpoint"
            } else {
                "onnx"
            },
            render_dpi: render_config.dpi,
            confidence_threshold: ocr_config.confidence_threshold,
        });
        #[cfg(not(feature = "ocr"))]
        let ocr_info = None;

        let document = self.metadata.then(|| TextDocument {
            file_name: Path::new(&self.path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown"),
            info: DocumentInfo::from(&doc.metadata),
        });

        let stdout = io::stdout();
        let mut output = BufWriter::new(stdout.lock());

        write_extraction(
            &mut output,
            &wanted,
            self.json,
            document,
            ocr_info,
            |page| Ok(doc.extract_text_layer(page)?),
            |page| {
                #[cfg(feature = "ocr")]
                {
                    let backend = match &mut ocr_backend {
                        Some(backend) => backend,
                        slot @ None => {
                            let backend: Box<dyn OcrBackend> = match &self.endpoint {
                                Some(endpoint) => Box::new(LlamaServerBackend::new(endpoint)),
                                None => Box::new(OnnxOcrBackend::from_env()?),
                            };

                            slot.insert(backend)
                        }
                    };

                    let image = doc.render_page(page, &render_config)?;
                    let result = backend.run(&image, page, &ocr_config)?;
                    Ok(result.text)
                }
                #[cfg(not(feature = "ocr"))]
                {
                    let _ = page;
                    Err("OCR support is disabled".into())
                }
            },
        )?;

        output.flush()?;
        Ok(())
    }
}

fn write_extraction(
    output: &mut impl Write,
    wanted: &[u32],
    json: bool,
    document: Option<TextDocument<'_>>,
    ocr: Option<OcrInfo>,
    mut native_text: impl FnMut(u32) -> Result<Option<String>, ExtractionError>,
    mut ocr_text: impl FnMut(u32) -> Result<String, ExtractionError>,
) -> Result<(), ExtractionError> {
    let mut json_output = Vec::new();
    let mut extraction = ExtractionInfo {
        ocr,
        ..ExtractionInfo::default()
    };

    if json {
        json_output.extend_from_slice(b"{\"pages\":[");
    }

    for (index, &page) in wanted.iter().enumerate() {
        let text = native_text(page)?;
        let (text, source, fallback_reason) = if ocr.is_some() && text.is_none() {
            let text = ocr_text(page)?;
            (
                (!text.trim().is_empty()).then_some(text),
                TextSource::Ocr,
                Some("no_native_text"),
            )
        } else {
            (text, TextSource::Native, None)
        };

        if document.is_some() {
            extraction.extracted_page_count += 1;
            match source {
                TextSource::Native => extraction.native_pages += 1,
                TextSource::Ocr => extraction.ocr_pages += 1,
            }
            extraction.empty_pages += usize::from(text.is_none());
        }

        let page_number = page + 1;
        if json {
            if index != 0 {
                json_output.push(b',');
            }
            let page = PageText {
                page: page_number,
                text: text.as_deref(),
            };
            if document.is_some() {
                serde_json::to_writer(
                    &mut json_output,
                    &MetadataPage {
                        page,
                        source,
                        fallback_reason,
                    },
                )?;
            } else {
                serde_json::to_writer(&mut json_output, &page)?;
            }
        } else {
            match text.as_deref() {
                Some(text) => {
                    writeln!(output, "=== page {page_number} ===\n{text}\n")?;
                }
                None => {
                    writeln!(output, "=== page {page_number} (no text extracted) ===\n")?;
                }
            }
        }
    }

    if json {
        json_output.extend_from_slice(b"]}\n");
        if let Some(document) = document {
            let mut header = Vec::new();
            header.extend_from_slice(b"{\"document\":");
            serde_json::to_writer(&mut header, &document)?;
            header.extend_from_slice(b",\"extraction\":");
            serde_json::to_writer(&mut header, &extraction)?;
            header.push(b',');
            output.write_all(&header)?;
            output.write_all(&json_output[1..])?;
        } else {
            output.write_all(&json_output)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde_json::{Value, json};

    use super::*;

    fn document() -> TextDocument<'static> {
        TextDocument {
            file_name: "document.pdf",
            info: DocumentInfo {
                page_count: 8,
                file_size_bytes: 245760,
                pdf_version: Some("1.7".to_owned()),
                title: Some("Example document"),
                author: None,
                subject: None,
                creator: None,
                producer: None,
            },
        }
    }

    fn ocr() -> OcrInfo {
        OcrInfo {
            fallback_enabled: true,
            backend: "onnx",
            render_dpi: 200,
            confidence_threshold: 0.5,
        }
    }

    #[rstest]
    #[case::plain(
        false,
        "=== page 2 ===\nNative text\n\n=== page 4 (no text extracted) ===\n\n"
    )]
    #[case::json(
        true,
        "{\"pages\":[{\"page\":2,\"text\":\"Native text\"},{\"page\":4,\"text\":null}]}\n"
    )]
    fn legacy_output_is_unchanged(#[case] json: bool, #[case] expected: &str) {
        let mut output = Vec::new();
        write_extraction(
            &mut output,
            &[1, 3],
            json,
            None,
            None,
            |page| Ok((page == 1).then(|| "Native text".to_owned())),
            |_| panic!("OCR must remain disabled"),
        )
        .unwrap();
        assert_eq!(output, expected.as_bytes());
    }

    #[rstest]
    #[case::native_only(false)]
    #[case::fallback(true)]
    fn provenance_and_counts(#[case] fallback: bool) {
        let mut output = Vec::new();
        let mut native_calls = Vec::new();
        let mut ocr_calls = Vec::new();
        write_extraction(
            &mut output,
            &[1, 3, 5],
            true,
            Some(document()),
            fallback.then_some(ocr()),
            |page| {
                native_calls.push(page);
                Ok((page == 1).then(|| "Native text".to_owned()))
            },
            |page| {
                ocr_calls.push(page);
                Ok(if page == 3 {
                    "Recognized text"
                } else {
                    " \n\t"
                }
                .to_owned())
            },
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(native_calls, [1, 3, 5]);
        assert_eq!(ocr_calls, if fallback { vec![3, 5] } else { vec![] });
        assert_eq!(
            value["document"],
            json!({
                "file_name": "document.pdf", "page_count": 8, "file_size_bytes": 245760,
                "pdf_version": "1.7", "title": "Example document", "author": null,
                "subject": null, "creator": null, "producer": null
            })
        );
        assert_eq!(
            value["extraction"],
            json!({
                "extracted_page_count": 3, "native_pages": if fallback { 1 } else { 3 },
                "ocr_pages": if fallback { 2 } else { 0 }, "empty_pages": if fallback { 1 } else { 2 },
                "ocr": fallback.then_some(ocr())
            })
        );
        assert_eq!(
            value["pages"],
            json!([
                {"page": 2, "text": "Native text", "source": "native", "fallback_reason": null},
                {"page": 4, "text": if fallback { Some("Recognized text") } else { None },
                 "source": if fallback { "ocr" } else { "native" },
                 "fallback_reason": if fallback { Some("no_native_text") } else { None }},
                {"page": 6, "text": null, "source": if fallback { "ocr" } else { "native" },
                 "fallback_reason": if fallback { Some("no_native_text") } else { None }}
            ])
        );
        let counts = &value["extraction"];
        assert_eq!(
            counts["native_pages"].as_u64().unwrap() + counts["ocr_pages"].as_u64().unwrap(),
            3
        );
        assert!(counts["empty_pages"].as_u64().unwrap() <= 3);
    }

    #[test]
    fn fallback_is_lazy_for_native_pages() {
        let mut output = Vec::new();
        write_extraction(
            &mut output,
            &[0, 1],
            true,
            Some(document()),
            Some(ocr()),
            |_| Ok(Some("Native text".to_owned())),
            |_| panic!("OCR initialization must remain lazy"),
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(
            value["extraction"]["ocr"],
            serde_json::to_value(ocr()).unwrap()
        );
        assert_eq!(value["extraction"]["native_pages"], 2);
        assert_eq!(value["extraction"]["ocr_pages"], 0);
    }

    #[rstest]
    #[case::native_legacy(false, false)]
    #[case::native_metadata(false, true)]
    #[case::ocr_legacy(true, false)]
    #[case::ocr_metadata(true, true)]
    fn extraction_errors_emit_no_json(#[case] ocr_error: bool, #[case] metadata: bool) {
        let mut output = Vec::new();
        let mut ocr_calls = 0;
        let result = write_extraction(
            &mut output,
            &[0, 1, 2],
            true,
            metadata.then(document),
            Some(ocr()),
            |page| match page {
                0 => Ok(Some("First page".to_owned())),
                1 if ocr_error => Ok(None),
                1 => Err("native extraction failed".into()),
                _ => panic!("extraction must stop at the error"),
            },
            |_| {
                ocr_calls += 1;
                Err("OCR failed".into())
            },
        );
        assert!(output.is_empty());
        assert_eq!(ocr_calls, usize::from(ocr_error));
        assert_eq!(
            result.unwrap_err().to_string(),
            if ocr_error {
                "OCR failed"
            } else {
                "native extraction failed"
            }
        );
    }
}
