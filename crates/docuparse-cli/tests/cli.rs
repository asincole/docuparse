#[test]
fn help_lists_all_commands() {
    let output = usage_rs::test::command!("docuparse-cli", "--help").assert_success();
    let stdout = output.stdout_text();

    for name in ["info", "text", "render", "ocr", "completion"] {
        assert!(stdout.contains(name), "missing '{name}' in help output");
    }
}

#[cfg(feature = "ocr")]
#[test]
fn text_help_lists_ocr_fallback() {
    let output = usage_rs::test::command!("docuparse-cli", "text", "--help").assert_success();

    assert!(output.stdout_text().contains("--ocr-fallback"));
}
