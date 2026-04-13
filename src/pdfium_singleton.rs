use std::{env, path::PathBuf, sync::Arc};

use once_cell::sync::OnceCell;
use pdfium_render::prelude::Pdfium;

use crate::error::PdfiumError;

static PDFIUM: OnceCell<Pdfium> = OnceCell::new();

/// Returns a Thread-safe `'static` reference to the lazily-initialised pdfium binding.
///
/// Reads `PDFIUM_LIB_PATH` from the environment — fails explicitly if unset.
/// The result is cached for the lifetime of the process; subsequent calls are
/// lock-free reads of the `OnceCell`.
pub fn get_or_init_pdfium() -> Result<&'static Pdfium, PdfiumError> {
    PDFIUM
        .get_or_try_init(|| {
            let lib_path = env::var("PDFIUM_LIB_PATH")
                .map(PathBuf::from)
                .map_err(|_| PdfiumError::MissingLibPath)?;

            Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(&lib_path))
                .map(Pdfium::new)
                .map_err(|source| PdfiumError::InitFailed {
                    source: Arc::new(source),
                })
        })
        .map_err(|e| e.clone())
}
