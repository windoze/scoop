use super::*;

#[test]
fn export_derives_the_only_strong_symbol_and_definition() {
    let fixture = Fixture::new("exported");
    let export = fixture.export();

    assert_eq!(
        export.expected_symbol().key(),
        PersistentSymbolKey::CallableBody(fixture.body)
    );
    assert_eq!(
        export.required_definition(),
        expected_definition(fixture.producer, fixture.body)
    );
    let section =
        CrossConeLirBridgeSectionV1::try_new(&fixture.foundation, vec![export.clone()], Vec::new())
            .unwrap();
    assert_eq!(section.export(fixture.declaration), Some(&export));
}

#[test]
fn export_requires_its_body_in_the_same_provider_foundation() {
    let fixture = Fixture::new("missingBody");
    let foundation = empty_foundation(fixture.producer);

    assert!(matches!(
        CrossConeLirBridgeSectionV1::try_new(
            &foundation,
            vec![fixture.export()],
            Vec::new(),
        ),
        Err(CrossConeLirBridgeBuildError::Relation(
            CrossConeLirBridgeRelationError::MissingExportBody { body, .. }
        )) if body == fixture.body
    ));
}

#[test]
fn root_plan_must_equal_the_canonical_abi_gc_effect() {
    let fixture = Fixture::new("wrongRoot");

    assert!(matches!(
        ParamFreeLirCallableExportV1::new(
            fixture.producer,
            fixture.declaration,
            fixture.target,
            fixture.abi,
            CallingConvention::Cdecl,
            DependencyExternalCallableRootPlanV1::ManagedStatepoint,
        ),
        Err(ParamFreeLirCallableBuildError::RootProtocolMismatch {
            abi: GcEffect::NoGc,
            root: GcEffect::Managed,
            ..
        })
    ));
}

#[test]
fn selected_callable_cannot_name_current_or_trusted_core() {
    let fixture = Fixture::new("selected");
    let selected_current = SelectedDependencyLirCallableV1::new(
        fixture.producer,
        fixture.declaration,
        fixture.target,
        fixture.abi.clone(),
        CallingConvention::Cdecl,
        DependencyExternalCallableRootPlanV1::NoGc,
    )
    .unwrap();
    assert!(matches!(
        CrossConeLirBridgeSectionV1::try_new(
            &fixture.foundation,
            Vec::new(),
            vec![selected_current],
        ),
        Err(CrossConeLirBridgeBuildError::Relation(
            CrossConeLirBridgeRelationError::SelectedCurrentProvider { .. }
        ))
    ));

    let consumer = ConeCoordinate::new("test", "consumer", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let selected_core = SelectedDependencyLirCallableV1::new(
        ConeIdentity::CORE,
        fixture.declaration,
        fixture.target,
        fixture.abi,
        CallingConvention::Cdecl,
        DependencyExternalCallableRootPlanV1::NoGc,
    )
    .unwrap();
    assert!(matches!(
        CrossConeLirBridgeSectionV1::try_new(
            &empty_foundation(consumer),
            Vec::new(),
            vec![selected_core],
        ),
        Err(CrossConeLirBridgeBuildError::Relation(
            CrossConeLirBridgeRelationError::SelectedTrustedCore { .. }
        ))
    ));
}

#[test]
fn trusted_core_uses_only_its_dedicated_bridge() {
    let fixture = Fixture::new("notCoreExport");

    assert!(matches!(
        CrossConeLirBridgeSectionV1::try_new(
            &empty_foundation(ConeIdentity::CORE),
            vec![fixture.export()],
            Vec::new(),
        ),
        Err(CrossConeLirBridgeBuildError::Relation(
            CrossConeLirBridgeRelationError::CoreExportsOrdinaryDependencyCallable
        ))
    ));
}
