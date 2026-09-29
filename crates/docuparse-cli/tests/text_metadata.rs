use std::{
    path::Path,
    process::{Command, Output},
};

use lopdf::{
    Document, Object, Stream,
    content::{Content, Operation},
    dictionary,
};
use serde_json::{Value, json};

fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_docuparse-cli"));
    command.current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."));
    command.env("HOTPATH_METRICS_SERVER_OFF", "1");
    command.env(
        "HOTPATH_OUTPUT_PATH",
        if cfg!(windows) { "NUL" } else { "/dev/null" },
    );
    command
}

fn fixture(path: &Path) {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let font = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica"
    });
    let resources = doc.add_object(dictionary! { "Font" => dictionary! { "F1" => font } });
    let pages: Vec<Object> = [Some("Native text"), None, Some("Last page")]
        .into_iter()
        .map(|text| {
            let operations = text.map_or_else(Vec::new, |text| {
                vec![
                    Operation::new("BT", vec![]),
                    Operation::new("Tf", vec!["F1".into(), 12.into()]),
                    Operation::new("Td", vec![50.into(), 700.into()]),
                    Operation::new("Tj", vec![Object::string_literal(text)]),
                    Operation::new("ET", vec![]),
                ]
            });
            let content = doc.add_object(Stream::new(
                dictionary! {},
                Content { operations }.encode().unwrap(),
            ));
            doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id, "Resources" => resources,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()], "Contents" => content
        }).into()
        })
        .collect();
    doc.objects.insert(
        pages_id,
        dictionary! { "Type" => "Pages", "Kids" => pages, "Count" => 3 }.into(),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    let info = doc.add_object(dictionary! {
        "Title" => Object::string_literal("Example document"),
        "Author" => Object::string_literal("Ada"),
        "Keywords" => Object::string_literal("fixture ".repeat(32))
    });
    doc.trailer.set("Root", catalog);
    doc.trailer.set("Info", info);
    doc.save(path).unwrap();
}

fn success(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn metadata_requires_json_before_loading_document() {
    let output = command()
        .args(["text", "--metadata", "missing.pdf"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("required arguments") && error.contains("--json"),
        "{error}"
    );
}

#[test]
fn help_describes_metadata_requirement() {
    let output = command().args(["text", "--help"]).output().unwrap();
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(
        help.contains("--metadata") && help.contains("requires --json"),
        "{help}"
    );
}

#[test]
fn selected_pages_reuse_metadata_and_preserve_legacy_contracts() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("document.pdf");
    fixture(&path);
    let enriched = success(
        command()
            .args(["text", "--json", "--metadata", "--pages", "3,1-2,1"])
            .arg(&path)
            .output()
            .unwrap(),
    );
    assert_eq!(
        enriched["document"],
        json!({
            "file_name": "document.pdf", "page_count": 3, "file_size_bytes": path.metadata().unwrap().len(),
            "pdf_version": "1.7", "title": "Example document", "author": "Ada",
            "subject": null, "creator": null, "producer": null
        })
    );
    assert_eq!(
        enriched["extraction"],
        json!({
            "extracted_page_count": 3, "native_pages": 3, "ocr_pages": 0, "empty_pages": 1, "ocr": null
        })
    );
    assert_eq!(
        enriched["pages"],
        json!([
            {"page": 1, "text": "Native text", "source": "native", "fallback_reason": null},
            {"page": 2, "text": null, "source": "native", "fallback_reason": null},
            {"page": 3, "text": "Last page", "source": "native", "fallback_reason": null}
        ])
    );
    let selected = success(
        command()
            .args(["text", "--json", "--metadata", "--pages", "3"])
            .arg(&path)
            .output()
            .unwrap(),
    );
    assert_eq!(selected["document"]["page_count"], 3);
    assert_eq!(selected["extraction"]["extracted_page_count"], 1);
    assert_eq!(selected["pages"][0]["page"], 3);

    let legacy = command()
        .args(["text", "--json"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(legacy.status.success());
    assert_eq!(legacy.stdout, b"{\"pages\":[{\"page\":1,\"text\":\"Native text\"},{\"page\":2,\"text\":null},{\"page\":3,\"text\":\"Last page\"}]}\n");
    let plain = command().arg("text").arg(&path).output().unwrap();
    assert!(plain.status.success());
    assert_eq!(plain.stdout, b"=== page 1 ===\nNative text\n\n=== page 2 (no text extracted) ===\n\n=== page 3 ===\nLast page\n\n");

    let info = success(
        command()
            .args(["info", "--json"])
            .arg(&path)
            .output()
            .unwrap(),
    );
    assert_eq!(
        info,
        json!({
            "pdf_name": "document", "page_count": 3, "file_size_bytes": path.metadata().unwrap().len(),
            "pdf_version": "Pdf1_7", "title": "Example document", "author": "Ada",
            "subject": null, "creator": null, "producer": null
        })
    );
}

#[cfg(feature = "ocr")]
#[test]
fn native_pages_do_not_initialize_either_ocr_backend() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("native.pdf");
    fixture(&path);
    for backend in ["onnx", "ocr_endpoint"] {
        let mut command = command();
        command.args(["text", "--json", "--metadata", "--pages", "1,3"]);
        if backend == "onnx" {
            command.arg("--ocr-fallback");
            command
                .env_remove("OCR_DET_MODEL_PATH")
                .env_remove("OCR_REC_MODEL_PATH")
                .env_remove("OCR_DICT_PATH");
        } else {
            command.args(["--endpoint", "invalid endpoint that must never be used"]);
        }
        let value = success(command.arg(&path).output().unwrap());
        assert_eq!(
            value["extraction"]["ocr"],
            json!({
                "fallback_enabled": true, "backend": backend, "render_dpi": 200, "confidence_threshold": 0.5
            })
        );
        assert_eq!(value["extraction"]["native_pages"], 2);
        assert_eq!(value["extraction"]["ocr_pages"], 0);
    }
}

#[test]
fn invalid_selection_emits_no_partial_json() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("document.pdf");
    fixture(&path);
    let output = command()
        .args(["text", "--json", "--metadata", "--pages", "1,4"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("out of range"));
}

#[cfg(feature = "ocr")]
#[test]
fn fallback_error_after_native_page_emits_no_partial_json() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("document.pdf");
    fixture(&path);
    let output = command()
        .args([
            "text",
            "--json",
            "--metadata",
            "--endpoint",
            "invalid endpoint",
        ])
        .arg(&path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}
