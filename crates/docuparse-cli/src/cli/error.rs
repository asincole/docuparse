/// Prints an error to stderr and exits with status 1.
///
/// Every docuparse call in this CLI routes failures through here so each
/// command's `run()` stays linear instead of threading `Result` through
/// the `Run` trait's `Output`.
pub(crate) fn fail(e: impl std::fmt::Display) -> ! {
    eprintln!("error: {e}");
    std::process::exit(1);
}
