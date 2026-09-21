use super::*;

pub(super) fn assert_initialization_and_dependency_calls(
    target: &scoop_toolchain::ResolvedTargetProfile,
    workspace: &Path,
    core: &Path,
) {
    for (name, text) in [
        (
            "initialization_call",
            include_str!(
                "../../../../../../../tests/fixtures/core-library/initialization-call.scoop"
            ),
        ),
        (
            "initialization_call_combined",
            include_str!(
                "../../../../../../../tests/fixtures/core-library/initialization-call-combined.scoop"
            ),
        ),
    ] {
        let source = workspace.join(format!("{name}.scoop"));
        std::fs::write(&source, text).unwrap();
        for (kind, label) in [
            (StageDumpKind::Hir, "hir"),
            (StageDumpKind::Mir, "mir"),
            (StageDumpKind::Lir, "lir"),
        ] {
            let output = build_consumer_emitting(
                target,
                &source,
                &workspace.join(format!("{name}-{label}.slib")),
                core,
                StageDumpPolicy::Stage(kind),
            );
            let dump = output.emitted_dump().unwrap();
            if kind == StageDumpKind::Lir {
                assert!(
                    dump.text().contains("external-fn0"),
                    "initialization protocol has its shared arena entry"
                );
                assert!(
                    dump.text().contains("external-fn1"),
                    "ordinary dependency has a distinct shared arena entry"
                );
                assert!(!dump.text().contains("core-external-fn"));
                assert!(!dump.text().contains("dependency-external-fn"));
            }
            insta::assert_snapshot!(format!("{name}_{label}"), dump.text());
        }
    }
}
