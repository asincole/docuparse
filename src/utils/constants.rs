pub const KB: u64 = 1024;
pub const MB: u64 = 1024 * KB;
pub const PDF_MAGIC: &[u8; 5] = b"%PDF-";

/// Minimum valid PDF size — anything smaller is corrupt or empty.
pub const MIN_PDF_SIZE: u64 = 1 * KB;

/// Maximum accepted PDF size.
pub const MAX_PDF_SIZE: u64 = 500 * MB;
