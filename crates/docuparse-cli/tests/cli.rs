#[test]
fn help_lists_all_commands() {
    let output = usage_rs::test::command!("docuparse-cli", "--help").assert_success();
    let stdout = output.stdout_text();

    let names = [
        "info",
        "text",
        "render",
        "completion",
        #[cfg(feature = "ocr")]
        "ocr",
    ];

    for name in names {
        assert!(stdout.contains(name), "missing '{name}' in help output");
    }
}

#[cfg(feature = "ocr")]
#[test]
fn text_help_lists_ocr_fallback_options() {
    let output = usage_rs::test::command!("docuparse-cli", "text", "--help").assert_success();

    let stdout = output.stdout_text();
    assert!(stdout.contains("--ocr-fallback"), "missing --ocr-fallback");
    assert!(stdout.contains("--endpoint"), "missing --endpoint");
}
