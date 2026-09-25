use super::*;

#[test]
fn core_source_foundation_does_not_grant_odr_execution() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/core-source-odr.scoop"
    ));
    let used = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/core-source-odr-use.scoop"
    ));
    for (suffix, requested) in [("", false), (used, true)] {
        let output = support::lower_extra(&format!("{source}\n{suffix}"));
        Production::from_hir(&output).unwrap();
        let canonical = hir::CanonicalHirFoundation::from_modules(
            &output.export,
            &output.local,
            &output.native_boundary_types,
        )
        .unwrap();
        let result = hir::OdrFreeHirFoundation::try_new(canonical);
        if requested {
            assert!(matches!(
                result,
                Err(hir::OdrFreeHirFoundationError::CallableApplication(_))
            ));
        } else {
            result.unwrap();
        }
    }
}
