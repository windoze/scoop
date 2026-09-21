use super::*;

pub(super) fn check(output: &hir::Output, mir: &scoop_mir::Module) {
    assert_eq!(
        (
            selected(&hir::dump(&output.export)),
            selected(&scoop_mir::dump(mir))
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-defaults/generic-closure-expansion.hir.snap"
            ))
            .to_owned(),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-defaults/generic-closure-expansion.mir.snap"
            ))
            .to_owned()
        )
    );
}

fn selected(dump: &str) -> String {
    let mut keep = false;
    let mut result = String::new();
    for line in dump.lines() {
        if line.starts_with("  ") && !line.starts_with("   ") {
            keep = [
                "referenceProvider",
                "referenceBridge",
                "forwardReference",
                "lambdaProvider",
                "anonymousProvider",
                "closureBridge",
                "localReference",
                "forwardLocal",
                "boundMember",
                "boundExtension",
                "ReferenceConstructor",
                "fun main",
                "$local.",
                "$lambda.",
                "$anonymous.",
                "$reference.",
            ]
            .iter()
            .any(|name| line.contains(name))
                || line.starts_with("  closure ");
        }
        if keep {
            result.push_str(line);
            result.push('\n');
        }
    }
    result
}
