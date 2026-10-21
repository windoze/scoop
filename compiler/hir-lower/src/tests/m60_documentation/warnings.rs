use super::*;

#[test]
fn m60_documentation_warns_at_exact_source_spans_without_changing_compilation() {
    let source = "/** 中文 [Missing]\r\n * @param absent Missing parameter.\r\n * @receiver Invalid receiver.\r\n * @throws Int Not throwable.\r\n */\r\npublic fun documented() {}\r\n";
    let core = support::trusted_core();
    let output = lower_with_core(&core, source, true).unwrap();
    let docs = output.output().documentation.as_ref().unwrap();
    assert_eq!(docs.warnings.len(), 4, "{:?}", docs.warnings);
    assert!(docs.warnings.iter().all(|warning| warning.severity
        == ast::DiagnosticSeverity::Warning
        && warning.source.as_deref() == Some(&test_source_identity("src/main.scoop"))));
    let warning = docs
        .warnings
        .iter()
        .find(|warning| warning.message.contains("`Missing`"))
        .unwrap();
    let span = warning.span.unwrap();
    assert_eq!(&source[span.start as usize..span.end as usize], "Missing");
    assert!(output.output().warnings.is_empty());
    let ordinary = lower_with_core(&core, source, false).unwrap();
    assert!(ordinary.output().documentation.is_none());
    assert_eq!(
        output.output().export.functions.len(),
        ordinary.output().export.functions.len()
    );
}

#[test]
fn m60_documentation_rejects_real_function_body_errors() {
    let errors = lower_documented("/** Fine documentation. */\npublic fun broken(): Int = true")
        .err()
        .expect("a real body error rejects documentation");
    assert!(
        errors
            .iter()
            .any(|error| error.severity == ast::DiagnosticSeverity::Error)
    );
}

#[test]
fn m60_documentation_does_not_bind_display_labels_as_real_parameters() {
    let output = lower_documented(
        r#"
        /** [receiver]
         * @param receiver No parameter has this name.
         */
        public fun Int.first(): Int = 1
        /**
         * @param receiver Actual value parameter.
         * @receiver The extension receiver.
         */
        public fun Int.second(receiver: Int): Int = receiver
        "#,
    )
    .unwrap();
    let docs = output.output().documentation.as_ref().unwrap();
    assert_eq!(docs.warnings.len(), 2, "{:?}", docs.warnings);
    let tags = &comment(&output, Kind::Function, "second").tags;
    assert!(matches!(
        tags[0].kind,
        Tag::Param(Target::Declaration(Source::Parameter { .. }))
    ));
    assert!(matches!(
        tags[1].kind,
        Tag::Receiver(Target::Declaration(Source::Receiver(_)))
    ));
}

#[test]
fn m60_documentation_diagnoses_duplicate_resolved_exception_and_see_tags() {
    let output = lower_documented(
        r#"
        /**
         * @throws Throwable First exception.
         * @throws Throwable Duplicate exception.
         * @see Int
         * @see Int
         * @see https://example.com
         * @see https://example.com
         */
        public fun documented() {}
        "#,
    )
    .unwrap();
    let docs = output.output().documentation.as_ref().unwrap();
    assert_eq!(docs.warnings.len(), 3, "{:?}", docs.warnings);
    assert!(
        docs.warnings
            .iter()
            .all(|warning| warning.message.contains("duplicate documentation tag"))
    );
    assert_eq!(comment(&output, Kind::Function, "documented").tags.len(), 3);
}
