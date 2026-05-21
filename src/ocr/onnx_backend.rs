use std::{env, path::PathBuf, sync::Arc};

use image::DynamicImage;
use oar_ocr::{
    domain::TextDetectionConfig,
    oarocr::{OAROCR, OAROCRBuilder, OAROCRResult},
};

use crate::{
    OcrError,
    ocr::{OcrBackend, OcrConfig, OcrPageResult},
};

fn resolve_env_path(var: &'static str) -> Result<PathBuf, OcrError> {
    env::var(var)
        .map(PathBuf::from)
        .map_err(|_| OcrError::MissingEnvVar { var })
}

/// Assembles a confidence-filtered plain-text string from raw inference output.
fn assemble_text(raw: &OAROCRResult, threshold: f32) -> String {
    raw.text_regions
        .iter()
        .filter_map(|region| {
            let (text, confidence) = region.text_with_confidence()?;
            (confidence > threshold).then_some(text)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// ONNX OCR backend backed by `oar-ocr` and `ort`.
///
/// Wraps `Arc<OAROCR>` internally - cloning is O(1).
/// Construct once and share via `Arc<dyn OcrBackend>` across documents.
///
/// Model paths are resolved from environment variables by [`OnnxOcrBackend::from_env`]:
///
/// | Variable             | Artifact                    |
/// |----------------------|-----------------------------|
/// | `OCR_DET_MODEL_PATH` | Text detection ONNX model   |
/// | `OCR_REC_MODEL_PATH` | Text recognition ONNX model |
/// | `OCR_DICT_PATH`      | Character dictionary `.txt` |
///
/// # Thread safety
///
/// ONNX Runtime guarantees concurrent `Run()` calls on the same session are
/// safe. The manual `Send + Sync` impls below reflect that contract - revisit
/// if `OAROCR` ever wraps non-`Send` state.
#[derive(Debug, Clone)]
pub struct OnnxOcrBackend(Arc<OAROCR>);

// SAFETY: ONNX Runtime C API guarantees concurrent Run() calls on a single
// session are safe. `Arc` provides shared ownership without mutable aliasing.
unsafe impl Send for OnnxOcrBackend {}
unsafe impl Sync for OnnxOcrBackend {}

impl OnnxOcrBackend {
    /// Construct from explicit model artifact paths.
    ///
    /// ORT session compilation happens here - typically 500ms–2s.
    /// Construct once and share via `Arc`.
    pub fn new(
        det_model: impl Into<PathBuf>,
        rec_model: impl Into<PathBuf>,
        dict: impl Into<PathBuf>,
    ) -> Result<Self, OcrError> {
        let det_config = TextDetectionConfig {
            limit_side_len: Some(736),
            score_threshold: 0.3,
            box_threshold: 0.6,
            unclip_ratio: 2.0,
            ..Default::default()
        };

        let inner = OAROCRBuilder::new(det_model.into(), rec_model.into(), dict.into())
            .text_detection_config(det_config)
            .image_batch_size(1)
            .region_batch_size(8)
            .return_word_box(false)
            .build()
            .map_err(|e| OcrError::BackendBuildFailed {
                source: Arc::new(e),
            })?;

        Ok(Self(Arc::new(inner)))
    }

    /// Construct from environment variables. See [`OnnxOcrBackend`] for variable names.
    pub fn from_env() -> Result<Self, OcrError> {
        Self::new(
            resolve_env_path("OCR_DET_MODEL_PATH")?,
            resolve_env_path("OCR_REC_MODEL_PATH")?,
            resolve_env_path("OCR_DICT_PATH")?,
        )
    }

    /// Full inference result with bounding boxes and per-region confidence scores.
    ///
    /// Use when spatial layout data is required. For plain-text extraction
    /// prefer [`OcrBackend::run`] which is backend-agnostic.
    pub fn run_detailed(
        &self,
        image: &DynamicImage,
        page_index: u32,
    ) -> Result<OAROCRResult, OcrError> {
        let rgb = image.to_rgb8();

        if rgb.width() == 0 || rgb.height() == 0 {
            return Err(OcrError::ImageConversionFailed { page: page_index });
        }

        let mut results = self
            .0
            .predict(vec![rgb])
            .map_err(|e| OcrError::InferenceFailed {
                page: page_index,
                source: Arc::new(e),
            })?;

        results.pop().ok_or(OcrError::InferenceFailed {
            page: page_index,
            source: Arc::new(std::io::Error::other(
                "ONNX backend returned empty result set",
            )),
        })
    }
}

impl OcrBackend for OnnxOcrBackend {
    fn run(
        &self,
        image: &DynamicImage,
        page_index: u32,
        config: &OcrConfig,
    ) -> Result<OcrPageResult, Box<dyn std::error::Error + Send + Sync>> {
        let raw = self.run_detailed(image, page_index)?;
        let text = assemble_text(&raw, config.confidence_threshold);
        Ok(OcrPageResult { page_index, text })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_env_path_missing_var_yields_missing_env_var() {
        let err = resolve_env_path("DOCUPARSE_TEST_NONEXISTENT_VAR_XYZ").unwrap_err();
        assert!(
            matches!(
                err,
                OcrError::MissingEnvVar {
                    var: "DOCUPARSE_TEST_NONEXISTENT_VAR_XYZ"
                }
            ),
            "got: {err:?}",
        );
    }

    #[test]
    fn resolve_env_path_set_var_returns_path() {
        // SAFETY: test binary is single-threaded at this point.
        unsafe { env::set_var("DOCUPARSE_TEST_PATH_VAR", "/some/model.onnx") };
        let path = resolve_env_path("DOCUPARSE_TEST_PATH_VAR").unwrap();
        assert_eq!(path, PathBuf::from("/some/model.onnx"));
        unsafe { env::remove_var("DOCUPARSE_TEST_PATH_VAR") };
    }
}
