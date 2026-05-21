use std::{env, path::PathBuf, sync::Arc};

use once_cell::sync::OnceCell;
use pdfium_render::prelude::Pdfium;

#[derive(Debug, thiserror::Error)]
pub(crate) enum InitError {
    #[error("PDFIUM_LIB_PATH environment variable is not set")]
    MissingLibPath,

    #[error("PDFium library could not be initialised")]
    BindFailed {
        #[source]
        source: Arc<dyn std::error::Error + Send + Sync>,
    },
}

static PDFIUM: OnceCell<Pdfium> = OnceCell::new();

/// Returns a Thread-safe `'static` reference to the lazily-initialised pdfium binding.
///
/// Reads `PDFIUM_LIB_PATH` from the environment - fails explicitly if unset.
/// The result is cached for the lifetime of the process; subsequent calls are
/// lock-free reads of the `OnceCell`.
pub(crate) fn get_or_init_pdfium() -> Result<&'static Pdfium, InitError> {
    PDFIUM.get_or_try_init(init_pdfium)
}

fn init_pdfium() -> Result<Pdfium, InitError> {
    let lib_path = env::var("PDFIUM_LIB_PATH")
        .map(PathBuf::from)
        .map_err(|_| InitError::MissingLibPath)?;

    Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(&lib_path))
        .map(Pdfium::new)
        .map_err(|e| InitError::BindFailed {
            source: Arc::new(e),
        })
}

#[cfg(test)]
mod tests {
    use std::{error::Error, sync::Mutex};

    use super::*;

    // Serialises all env-mutating tests within this process.
    // POSIX getenv is not thread-safe - without this lock, concurrent
    // set_var/remove_var calls are undefined behaviour on Linux.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    // All tests target `init_pdfium` directly, not `get_or_init_pdfium`.
    //
    // `get_or_init_pdfium` wraps a process-global OnceCell - once it
    // succeeds it caches the Pdfium binding forever and the init path
    // becomes unreachable for the rest of the process. Testing through
    // the wrapper would require a real pdfium .so on every CI machine
    // and would make the success/failure paths order-dependent across
    // tests. `init_pdfium` is a plain function with no global side
    // effects and can be called freely.

    #[test]
    fn missing_env_var_yields_missing_lib_path_variant() {
        let _guard = ENV_LOCK.lock().unwrap();
        // SAFETY: no other thread reads the environment - enforced by ENV_LOCK.
        unsafe { env::remove_var("PDFIUM_LIB_PATH") };

        let err = init_pdfium().unwrap_err();

        assert!(
            matches!(err, InitError::MissingLibPath),
            "expected MissingLibPath, got: {err:?}",
        );
    }

    #[test]
    fn missing_env_var_display_names_the_variable() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { env::remove_var("PDFIUM_LIB_PATH") };

        let msg = init_pdfium().unwrap_err().to_string();

        assert!(
            msg.contains("PDFIUM_LIB_PATH"),
            "display must name the missing variable so the user knows what to set, got: {msg:?}",
        );
    }

    #[test]
    fn missing_env_var_has_no_source() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { env::remove_var("PDFIUM_LIB_PATH") };

        let err = init_pdfium().unwrap_err();

        // MissingLibPath is a leaf error - there is no underlying cause to
        // chain. A source here would be misleading.
        assert!(
            err.source().is_none(),
            "MissingLibPath should have no source, got: {:?}",
            err.source(),
        );
    }

    #[test]
    fn nonexistent_path_yields_bind_failed_variant() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { env::set_var("PDFIUM_LIB_PATH", "/nonexistent/path/that/cannot/exist") };

        let err = init_pdfium().unwrap_err();

        assert!(
            matches!(err, InitError::BindFailed { .. }),
            "expected BindFailed, got: {err:?}",
        );

        unsafe { env::remove_var("PDFIUM_LIB_PATH") };
    }

    #[test]
    fn bind_failed_carries_source() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { env::set_var("PDFIUM_LIB_PATH", "/nonexistent/path/that/cannot/exist") };

        let err = init_pdfium().unwrap_err();

        assert!(
            err.source().is_some(),
            "BindFailed must carry a source - loggers and error reporters \
             walk the chain to produce full diagnostics",
        );

        unsafe { env::remove_var("PDFIUM_LIB_PATH") };
    }

    #[test]
    fn bind_failed_display_does_not_duplicate_source_message() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { env::set_var("PDFIUM_LIB_PATH", "/nonexistent/path/that/cannot/exist") };

        let err = init_pdfium().unwrap_err();
        let display = err.to_string();
        let source_display = err.source().map(|s| s.to_string()).unwrap_or_default();

        assert!(
            !display.contains(&source_display) || source_display.is_empty(),
            "Display must not duplicate source message.\n\
             display:        {display:?}\n\
             source display: {source_display:?}",
        );

        unsafe { env::remove_var("PDFIUM_LIB_PATH") };
    }

    #[test]
    fn bind_failed_source_is_erased_to_dyn_error() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { env::set_var("PDFIUM_LIB_PATH", "/nonexistent/path/that/cannot/exist") };

        let err = init_pdfium().unwrap_err();

        let source = err.source().expect("BindFailed must have a source");
        let _msg = source.to_string(); // must not require knowing the concrete type

        unsafe { env::remove_var("PDFIUM_LIB_PATH") };
    }
}
