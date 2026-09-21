use super::*;

pub(super) fn assert_alias_stage_dumps(
    target: &scoop_toolchain::ResolvedTargetProfile,
    workspace: &Path,
    core: &Path,
) {
    let source = workspace.join("aliases.scoop");
    std::fs::write(
        &source,
        include_str!("../../../../../../../tests/fixtures/core-library/alias-program.scoop"),
    )
    .unwrap();
    for (kind, label) in [
        (StageDumpKind::Hir, "core_alias_hir"),
        (StageDumpKind::Mir, "core_alias_mir"),
        (StageDumpKind::Lir, "core_alias_lir"),
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
