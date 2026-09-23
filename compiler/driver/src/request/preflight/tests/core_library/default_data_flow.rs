use super::*;

pub(super) fn assert_branching_defaults(
    target: &scoop_toolchain::ResolvedTargetProfile,
    workspace: &Path,
    core: &Path,
) {
    for (case, text) in [
        (
            "standalone",
            include_str!(
                "../../../../../../../tests/fixtures/core-library/default-data-flow-standalone.scoop"
            ),
        ),
        (
            "combined",
            include_str!(
                "../../../../../../../tests/fixtures/core-library/default-data-flow-combined.scoop"
            ),
        ),
    ] {
        let source = workspace.join(format!("default-data-flow-{case}.scoop"));
        std::fs::write(&source, text).unwrap();
        for (kind, stage) in [
            (StageDumpKind::Hir, "hir"),
            (StageDumpKind::Mir, "mir"),
            (StageDumpKind::Lir, "lir"),
        ] {
            let label = format!("default_data_flow_{case}_{stage}");
            let output = build_consumer_emitting(
                target,
                &source,
                &workspace.join(format!("{label}.slib")),
                core,
                StageDumpPolicy::Stage(kind),
            );
            let dump = output.emitted_dump().unwrap();
            assert_eq!(dump.kind(), kind);
            insta::assert_snapshot!(label, dump.text());
        }
    }
}
