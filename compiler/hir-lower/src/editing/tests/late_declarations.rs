use super::file_headers::available;
use super::imports::{analyze_files, at};
use super::*;

#[test]
fn errno_signature_failures_do_not_stop_independent_declarations() {
    for declaration in [
        "@Extern(name = \"broken\", captureErrno = true)\nfun broken(): Int",
        "@Extern(name = \"broken\", captureErrno = 1)\nfun broken(): Int",
        "@Extern(name = \"broken\", abi = \"scoop\", captureErrno = true)\nfun broken(): (Int, Int)",
    ] {
        let text = format!(
            "{declaration}\nfun independent(value: Boolean): Boolean = value\n@Extern(name = \"accepted\", captureErrno = true)\nfun accepted(): (Int, Int)"
        );
        let mut analysis = analyze_files(&[("src/errno.scoop", &text)]);
        assert!(
            available(&mut analysis, "independent"),
            "{:?}",
            analysis.diagnostics()
        );
        assert!(matches!(
            analysis.signature(function(&analysis, "broken")),
            V::Unavailable(_)
        ));
        assert!(matches!(
            analysis.signature(function(&analysis, "accepted")),
            V::Known(_)
        ));
        let origin = at("src/errno.scoop", &text, "broken():", 6);
        let index = analysis.reference_index(|| false).unwrap();
        assert!(index.targets_at(&origin).is_empty());
        assert!(!index.unavailable.is_empty());
    }
}

#[test]
fn failed_extern_variable_annotations_keep_their_name_and_source_barriers() {
    let text = "package capture\n@Extern(name = \"selected\", captureErrno = true)\nval selected: Int\nfun independent(value: Boolean): Boolean = value";
    let use_text = "import capture.selected as chosen\nfun uses(): Int = unsafe { chosen }";
    let mut analysis = analyze_files(&[
        ("src/errno.scoop", text),
        ("src/use.scoop", use_text),
        ("src/fallback.scoop", "const val chosen: Int = 1"),
    ]);
    assert!(
        available(&mut analysis, "independent"),
        "{:?}",
        analysis.diagnostics()
    );
    assert!(!available(&mut analysis, "uses"));
    let source = at("src/errno.scoop", text, "selected: Int", 8);
    assert!(matches!(
        analysis
            .property_declaration(&source.source, source.span, || false)
            .unwrap(),
        V::Unavailable(_)
    ));
    let imported = at("src/use.scoop", use_text, "chosen", 6);
    assert!(
        analysis
            .import_definitions(&imported.source, imported.span, || false)
            .unwrap()
            .is_empty()
    );
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.targets_at(&source).is_empty());
    assert!(index.targets_at(&imported).is_empty());
    let fallback = at(
        "src/fallback.scoop",
        "const val chosen: Int = 1",
        "chosen",
        6,
    );
    let target = index.targets_at(&fallback)[0];
    assert_eq!(index.references_to(target).count(), 1);
}
