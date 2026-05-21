mod constants;

pub(crate) use constants::*;

/// Returns `true` if `text` contains at least one whitespace-delimited token
/// that is purely alphabetic (Unicode-aware, handles ä/ö/ü/ß) and at least
/// `min_word_length` codepoints long.
///
/// Useful for distinguishing pages with a genuine native text layer from pages
/// where the layer exists but contains only garbage (corrupt encoding, OCR
/// baked into the PDF, symbol-only content).
///
/// # Example
///
/// ```rust
/// use docuparse::contains_real_words;
///
/// assert!(contains_real_words("Hello world", 3));
/// assert!(!contains_real_words("123 !@# ---", 3));
/// assert!(!contains_real_words("", 3));
/// ```
pub fn contains_real_words(text: &str, min_word_length: usize) -> bool {
    if text.trim().is_empty() {
        return false;
    }
    text.split_whitespace()
        .any(|w| w.len() >= min_word_length && w.chars().all(|c| c.is_alphabetic()))
}
