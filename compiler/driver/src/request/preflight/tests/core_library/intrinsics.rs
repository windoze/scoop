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
            StageDumpPolicy::Stage(kind),
        );
        let dump = output.emitted_dump().unwrap();
        assert_eq!(dump.kind(), kind);
        insta::assert_snapshot!(label, dump.text());
    }
}

pub(super) fn assert_integer_exception_requires_layout(
    target: &scoop_toolchain::ResolvedTargetProfile,
    workspace: &Path,
    core: &Path,
) {
    let source = workspace.join("integer-exception.scoop");
    std::fs::write(&source, include_str!("../../../../../../../tests/fixtures/core-library/integer-default-exception-consumer.scoop")).unwrap();
    let output = workspace.join("integer-exception.slib");
    let error = consumer_request(target, &source, &output, core, StageDumpPolicy::None)
        .build_and_publish(DecodeLimits::default())
        .unwrap_err();
    let SingleConeProductionError::Production(error) = error else {
        panic!("integer exception rejection must originate in HIR")
    };
    let CurrentConeProductionFailure::Hir(CurrentConeHirStageError::Lowering(diagnostics)) =
        error.cause()
    else {
        panic!("missing exception layout must be rejected before MIR: {error:?}")
    };
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].message,
        "SCOOP_HIR_CROSS_CONE_LAYOUT_REQUIRED: dependency integer division exception construction requires layout/ABI capability from M23-6"
    );
    let text = std::fs::read_to_string(&source).unwrap();
    let start = text.find("userCoreManagedIntegerDefault()").unwrap() as u32;
    assert_eq!(
        diagnostics[0].span,
        Some(scoop_ast::Span::new(
            start,
            start + "userCoreManagedIntegerDefault()".len() as u32
        ))
    );
    assert!(!output.exists());
}
