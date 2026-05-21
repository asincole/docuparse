#![cfg(feature = "ocr-onnx")]

use docuparse::ocr::{OcrBackend, OcrConfig, OnnxOcrBackend};
use image::DynamicImage;

// tests/fixtures/known_text.png contains:
//   "Invoice Number: 1234"
//   "Total Amount: 99.00"
const KNOWN_TEXT_FIXTURE: &str = "tests/fixtures/known_text.png";
const KNOWN_TEXT_LINE_1: &str = "Invoice Number";
const KNOWN_TEXT_LINE_2: &str = "Total Amount";

/// Full pipeline test - loads real model artifacts and runs inference on a
/// known fixture image. Asserts that specific text is present in the output.
///
/// Run with:
/// cargo nextest run --features ocr-onnx --run-ignored only
#[ignore = "requires OCR_DET_MODEL_PATH, OCR_REC_MODEL_PATH, OCR_DICT_PATH and tests/fixtures/known_text.png"]
#[test]
fn onnx_backend_extracts_known_text() {
    let backend = OnnxOcrBackend::from_env().expect("OCR_*_PATH env vars must be set");
    let image = image::open(KNOWN_TEXT_FIXTURE).expect("fixture missing - run generate_fixtures");
    let config = OcrConfig::builder().confidence_threshold(0.5).build();

    let result = backend.run(&image, 0, &config).unwrap();

    assert!(result.has_text(), "expected text, got empty result");
    assert!(
        result.text.contains(KNOWN_TEXT_LINE_1),
        "missing '{}' in:\n{}",
        KNOWN_TEXT_LINE_1,
        result.text
    );
    assert!(
        result.text.contains(KNOWN_TEXT_LINE_2),
        "missing '{}' in:\n{}",
        KNOWN_TEXT_LINE_2,
        result.text
    );
}

#[test]
fn onnx_backend_rejects_zero_dimension_image() {
    let Ok(backend) = OnnxOcrBackend::from_env() else {
        return;
    };

    let err = backend
        .run_detailed(&DynamicImage::ImageRgb8(image::RgbImage::new(0, 100)), 0)
        .unwrap_err();

    assert!(
        matches!(err, docuparse::OcrError::ImageConversionFailed { page: 0 }),
        "expected ImageConversionFailed {{ page: 0 }}, got: {err:?}",
    );
}
