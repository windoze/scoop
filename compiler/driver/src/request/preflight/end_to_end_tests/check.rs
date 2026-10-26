use super::*;

#[test]
fn check_completes_hir_without_machine_toolchains_or_output_paths() {
    let workspace = tempfile::tempdir().unwrap();
    copy_trusted_core_sources(workspace.path());
    let source = workspace.path().join("src/scoop.core");
    let make_request = |triple: &str| {
        crate::normalize_direct_check_request(
            &source,
            Vec::new(),
            Vec::new(),
            DiagnosticOutputPolicy::Structured,
            crate::DirectCheckOptions {
                target: Some(triple.into()),
                sysroot: Some(workspace.path().to_path_buf()),
                ..Default::default()
            },
        )
        .unwrap()
    };
    for triple in [
        "aarch64-apple-darwin",
        "x86_64-unknown-linux-gnu",
        "x86_64-unknown-linux-musl",
    ] {
        let request = make_request(triple);
        assert!(matches!(
            request.target,
            crate::request::target::SelectedTarget::Frontend(_)
        ));
        assert!(request.output.primary_path().is_none());
        let result = request.compile(false).unwrap();
        assert!(matches!(result, SingleConeCompilation::Checked(_)));
        assert!(result.warnings().is_empty());
        assert!(result.emitted_dumps().is_empty());
    }
    assert_eq!(std::fs::read_dir(workspace.path()).unwrap().count(), 1);

    std::fs::write(
        source.join("src/invalid.scoop"),
        "public fun invalidCheckBody(): Int = missingBodyValue\n",
    )
    .unwrap();
    let error = make_request("x86_64-unknown-linux-gnu")
        .compile(false)
        .unwrap_err();
    assert!(
        error
            .structured_diagnostics()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic.message().contains("missingBodyValue"))
    );
}
