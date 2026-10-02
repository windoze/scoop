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
            StageDumpPolicy::Stages(scoop_protocol::StageDumpSet::one(kind)),
        );
        let dump = output.emitted_dumps().first().unwrap();
        assert_eq!(dump.kind(), kind);
        insta::assert_snapshot!(label, dump.text());
    }
}

pub(super) fn assert_normalized_integer_defaults(
    target: &scoop_toolchain::ResolvedTargetProfile,
    workspace: &Path,
    core: &Path,
) {
    let source = workspace.join("integer-defaults.scoop");
    std::fs::write(
        &source,
        include_str!(
            "../../../../../../../tests/fixtures/core-library/integer-default-consumer.scoop"
        ),
    )
    .unwrap();
    for (kind, label) in [
        (StageDumpKind::Hir, "integer_defaults_hir"),
        (StageDumpKind::Mir, "integer_defaults_mir"),
        (StageDumpKind::Lir, "integer_defaults_lir"),
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

pub(super) fn assert_integer_exception_uses_shared_layout(
    target: &scoop_toolchain::ResolvedTargetProfile,
    workspace: &Path,
    core: &Path,
) {
    let source = workspace.join("integer-exception.scoop");
    std::fs::write(&source, include_str!("../../../../../../../tests/fixtures/core-library/integer-default-exception-consumer.scoop")).unwrap();
    let output = build_consumer_emitting(
        target,
        &source,
        &workspace.join("integer-exception.slib"),
        core,
        StageDumpPolicy::Stages(scoop_protocol::StageDumpSet::one(StageDumpKind::Mir)),
    );
    let dump = output.emitted_dumps().first().unwrap();
    assert_eq!(dump.kind(), StageDumpKind::Mir);
    insta::assert_snapshot!("integer_exception_mir", dump.text());
}
