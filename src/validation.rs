use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

use crate::{
    error::ValidationError,
    utils::{MAX_PDF_SIZE, MB, MIN_PDF_SIZE, PDF_MAGIC},
};

fn validate_metadata(meta: &fs::Metadata) -> Result<(), ValidationError> {
    let size = meta.len();

    if size < MIN_PDF_SIZE {
        return Err(ValidationError::TooSmall {
            size,
            min: MIN_PDF_SIZE,
        });
    }

    if size > MAX_PDF_SIZE {
        return Err(ValidationError::FileTooLarge {
            size_mb: size / MB,
            max_mb: MAX_PDF_SIZE / MB,
        });
    }

    Ok(())
}

fn validate_magic_bytes(file: &mut File, path: &Path) -> Result<(), ValidationError> {
    let mut header: [u8; 5] = [0u8; PDF_MAGIC.len()];

    file.read_exact(&mut header)
        .map_err(|source| ValidationError::Io {
            path: path.to_path_buf(),
            source,
        })?;

    if header != *PDF_MAGIC {
        return Err(ValidationError::InvalidMagicBytes);
    }

    Ok(())
}

pub fn validate_pdf(path: &Path) -> Result<(), ValidationError> {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(ValidationError::NotFound {
                path: path.to_path_buf(),
            });
        }
        Err(source) => {
            return Err(ValidationError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };

    let meta = file.metadata().map_err(|source| ValidationError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    if !meta.is_file() {
        return Err(ValidationError::NotAFile {
            path: path.to_path_buf(),
        });
    }

    validate_metadata(&meta)?;
    validate_magic_bytes(&mut file, path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs, io::Write, path::PathBuf};

    use rstest::{fixture, rstest};
    use tempfile::{NamedTempFile, TempDir};

    use super::*;

    // ── fixtures ─────────────────────────────────────────────────────────────────

    /// A valid, minimal PDF file on disk.
    #[fixture]
    fn valid_pdf() -> NamedTempFile {
        let mut f = NamedTempFile::new().expect("tempfile");
        f.write_all(PDF_MAGIC).expect("write magic");
        let padding = vec![0u8; (MIN_PDF_SIZE as usize).saturating_sub(PDF_MAGIC.len())];
        f.write_all(&padding).expect("write padding");
        f
    }

    /// A temp file whose content is deliberately not a PDF (wrong magic),
    /// but large enough to pass the size check.
    #[fixture]
    fn non_pdf_file() -> NamedTempFile {
        let mut f = NamedTempFile::new().expect("tempfile");
        let mut content = vec![0u8; MIN_PDF_SIZE as usize];
        let fake_magic = b"NOT_PDF!";
        content[..fake_magic.len()].copy_from_slice(fake_magic);
        f.write_all(&content).expect("write");
        f
    }

    /// A temp file that is too small (below MIN_PDF_SIZE), but has correct magic.
    #[fixture]
    fn too_small_pdf() -> NamedTempFile {
        let mut f = NamedTempFile::new().expect("tempfile");
        f.write_all(PDF_MAGIC).expect("write magic");
        f
    }

    /// A temp file that exceeds MAX_PDF_SIZE.
    #[fixture]
    fn too_large_pdf() -> NamedTempFile {
        let mut f = NamedTempFile::new().expect("tempfile");
        f.write_all(PDF_MAGIC).expect("write magic");
        let overflow = vec![0u8; (MAX_PDF_SIZE as usize) + 1];
        f.write_all(&overflow).expect("write overflow");
        f
    }

    /// A TempDir used to derive paths that simply do not exist.
    #[fixture]
    fn temp_dir() -> TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    // ── existence / file-type checks ─────────────────────────────────────────────
    #[rstest]
    fn test_missing_path() {
        let ghost = PathBuf::from("/tmp/__docuparse_ghost_path_that_cannot_exist__.pdf");
        let err = validate_pdf(&ghost).unwrap_err();
        assert!(
            matches!(err, ValidationError::NotFound { .. }),
            "expected NotFound, got {err:?}"
        );
    }

    #[rstest]
    fn test_path_is_directory() {
        let temp_dir = tempfile::tempdir().expect("tempdir");

        let err = validate_pdf(temp_dir.path()).unwrap_err();
        assert!(
            matches!(err, ValidationError::NotAFile { .. }),
            "expected NotAFile, got {err:?}"
        );
    }

    // ── size checks ───────────────────────────────────────────────────────────────

    #[rstest]
    fn test_too_small(too_small_pdf: NamedTempFile) {
        let err = validate_pdf(too_small_pdf.path()).unwrap_err();
        assert!(
            matches!(err, ValidationError::TooSmall { .. }),
            "expected TooSmall, got {err:?}"
        );
    }

    #[rstest]
    fn test_too_large(too_large_pdf: NamedTempFile) {
        let err = validate_pdf(too_large_pdf.path()).unwrap_err();
        assert!(
            matches!(err, ValidationError::FileTooLarge { .. }),
            "expected FileTooLarge, got {err:?}"
        );

        if let ValidationError::FileTooLarge { max_mb, .. } = err {
            assert_eq!(max_mb, MAX_PDF_SIZE / MB);
        }
    }

    // ── magic byte checks ─────────────────────────────────────────────────────────

    #[rstest]
    fn test_wrong_magic(non_pdf_file: NamedTempFile) {
        let err = validate_pdf(non_pdf_file.path()).unwrap_err();
        assert!(
            matches!(err, ValidationError::InvalidMagicBytes),
            "expected InvalidMagicBytes, got {err:?}"
        );
    }

    #[rstest]
    fn test_empty_file(temp_dir: TempDir) {
        // An empty file fails TooSmall before it ever reaches magic byte
        // validation — this is the correct pipeline behaviour.
        let empty = temp_dir.path().join("empty.pdf");
        fs::write(&empty, b"").expect("write empty");
        let err = validate_pdf(&empty).unwrap_err();
        assert!(
            matches!(err, ValidationError::TooSmall { .. }),
            "expected TooSmall on empty file, got {err:?}"
        );
    }

    // ── happy path ────────────────────────────────────────────────────────────────

    #[rstest]
    fn test_validate_pdf_happy_path(valid_pdf: NamedTempFile) {
        assert!(validate_pdf(valid_pdf.path()).is_ok());
    }

    // ── parametric: boundary values around MIN_PDF_SIZE ──────────────────────────

    #[rstest]
    #[case(MIN_PDF_SIZE - 1, true)]
    #[case(MIN_PDF_SIZE, false)]
    #[case(MIN_PDF_SIZE + 1, false)]
    fn test_size_boundary(#[case] size: u64, #[case] expect_err: bool, temp_dir: TempDir) {
        let path = temp_dir.path().join("boundary.pdf");
        let mut content = PDF_MAGIC.to_vec();
        let body_len = (size as usize).saturating_sub(PDF_MAGIC.len());
        content.extend(vec![0u8; body_len]);
        content.truncate(size as usize);
        fs::write(&path, &content).expect("write boundary file");

        let result = validate_pdf(&path);
        assert_eq!(
            result.is_err(),
            expect_err,
            "size={size}, expect_err={expect_err}, got={result:?}"
        );
    }

    // ── parametric: boundary values around MAX_PDF_SIZE ──────────────────────────

    #[rstest]
    #[case(MAX_PDF_SIZE - 1, false)]
    #[case(MAX_PDF_SIZE, false)]
    #[case(MAX_PDF_SIZE + 1, true)]
    fn test_max_size_boundary(#[case] size: u64, #[case] expect_err: bool, temp_dir: TempDir) {
        let path = temp_dir.path().join("max_boundary.pdf");
        let mut content = PDF_MAGIC.to_vec();
        let body_len = (size as usize).saturating_sub(PDF_MAGIC.len());
        content.extend(vec![0u8; body_len]);
        fs::write(&path, &content).expect("write max boundary file");

        let result = validate_pdf(&path);
        assert_eq!(
            result.is_err(),
            expect_err,
            "size={size}, expect_err={expect_err}, got={result:?}"
        );
    }
}
