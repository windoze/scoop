use super::*;

pub(super) fn assert_shared_intrinsic_constants(
    target: &scoop_toolchain::ResolvedTargetProfile,
    workspace: &Path,
    core: &Path,
) {
    let source = workspace.join("intrinsic-constants.scoop");
    std::fs::write(
        &source,
        include_str!(
            "../../../../../../../tests/fixtures/core-library/intrinsic-const-consumer.scoop"
        ),
    )
    .unwrap();
    for (kind, label) in [
        (StageDumpKind::Hir, "intrinsic_constants_hir"),
        (StageDumpKind::Mir, "intrinsic_constants_mir"),
        (StageDumpKind::Lir, "intrinsic_constants_lir"),
    ] {
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
