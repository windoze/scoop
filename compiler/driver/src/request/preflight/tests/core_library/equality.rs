use super::*;

pub(super) fn assert_equality(
    target: &scoop_toolchain::ResolvedTargetProfile,
    workspace: &Path,
    core: &Path,
) {
    let source = workspace.join("integer-equality.scoop");
    std::fs::write(&source, concat!(
        include_str!("../../../../../../../tests/fixtures/m23-imported-core-members/equality.scoop"),
        "\n",
        include_str!("../../../../../../../tests/fixtures/m23-imported-core-members/equality-patterns.scoop"),
        "\n",
        include_str!("../../../../../../../tests/fixtures/m23-imported-core-members/equality-combined.scoop"),
    )).unwrap();
    for (kind, label) in [
        (StageDumpKind::Hir, "integer_equality_hir"),
        (StageDumpKind::Mir, "integer_equality_mir"),
        (StageDumpKind::Lir, "integer_equality_lir"),
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
