use super::*;

pub(super) fn assert_member_calls(
    target: &scoop_toolchain::ResolvedTargetProfile,
    workspace: &Path,
    core: &Path,
) {
    let source = workspace.join("member-calls.scoop");
    std::fs::write(
        &source,
        concat!(
            include_str!(
                "../../../../../../../tests/fixtures/m23-imported-core-members/methods.scoop"
            ),
            "\n",
            include_str!(
                "../../../../../../../tests/fixtures/m23-imported-core-members/combined.scoop"
            ),
        ),
    )
    .unwrap();
    for (kind, label) in [
        (StageDumpKind::Hir, "member_calls_hir"),
        (StageDumpKind::Mir, "member_calls_mir"),
        (StageDumpKind::Lir, "member_calls_lir"),
    ] {
        let output = build_consumer_emitting(
            target,
            &source,
            &workspace.join(format!("{label}.slib")),
            core,
            StageDumpPolicy::Stages(scoop_protocol::StageDumpSet::one(kind)),
        );
        let dump = output.emitted_dumps().first().unwrap();
        assert_eq!(dump.kind(), kind);
        insta::assert_snapshot!(label, dump.text());
    }
}
