use super::*;
use scoop_identity::{
    CanonicalIdentifier, DeclarationScope, DefinitionOwnerChain, PackagePath,
    SourceDeclarationSite, SourceNominalKind,
};

fn source(provider: ConeIdentity, kind: SourceNominalKind) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            provider,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("Subject").unwrap(),
        kind,
        0,
    )
}

#[test]
fn core_and_ordinary_plans_preserve_the_actual_definition_provider() {
    for provider in [ConeIdentity::CORE, ConeIdentity::SINGLE_FILE] {
        for kind in [SourceNominalKind::Struct, SourceNominalKind::Interface] {
            let fixture = finish_fixture(source_fixture_parts(source(provider, kind)), None);
            let plan = ParamFreeShapeSupportPlanSetV1::from_sources(
                [&fixture.source],
                &fixture.foundation,
                &fixture.registrations,
            )
            .unwrap();
            let closure = &plan.closures()[0];
            assert_eq!(closure.root(), provider);
            assert_eq!(
                closure.roles().boxed_value().available().is_some(),
                kind == SourceNominalKind::Struct
            );
            let descriptor = closure.roles().type_descriptor().available().unwrap();
            let expected_definition = ObjectDefinitionPlanKey::strong(
                provider,
                StrongDefinitionEntity::exact_type(closure.owner()),
                StrongDefinitionRole::TypeDescriptor,
            )
            .unwrap();
            assert_eq!(
                descriptor.definition_plan(),
                scoop_identity::ObjectDefinitionPlanId::from_key(&expected_definition).unwrap()
            );
            let bytes = encode(&plan).unwrap();
            let decoded: DecodedParamFreeShapeSupportPlanSetV1 = decode_canonical(&bytes).unwrap();
            let replayed = decoded
                .validate(
                    [&fixture.source],
                    &mut source_graph(&fixture.source),
                    &fixture.foundation,
                    &fixture.registrations,
                )
                .unwrap();
            assert_eq!(replayed, plan);
            assert_eq!(encode(&replayed).unwrap(), bytes);
        }
    }
}

#[test]
fn ordinary_reader_rejects_wrong_provider_and_inexact_source_coverage() {
    let fixture = finish_fixture(
        source_fixture_parts(source(ConeIdentity::SINGLE_FILE, SourceNominalKind::Struct)),
        None,
    );
    let plan = ParamFreeShapeSupportPlanSetV1::from_sources(
        [&fixture.source],
        &fixture.foundation,
        &fixture.registrations,
    )
    .unwrap();
    let bytes = encode(&plan).unwrap();
    let mut wrong_root: DecodedParamFreeShapeSupportPlanSetV1 = decode_canonical(&bytes).unwrap();
    wrong_root.closures[0].root = decode_canonical(&encode(&ConeIdentity::CORE).unwrap()).unwrap();
    assert!(matches!(
        wrong_root.validate(
            [&fixture.source],
            &mut source_graph(&fixture.source),
            &fixture.foundation,
            &fixture.registrations,
        ),
        Err(ParamFreeShapeSupportValidationError::WrongRoot)
    ));
    let extra: DecodedParamFreeShapeSupportPlanSetV1 = decode_canonical(&bytes).unwrap();
    assert!(matches!(
        extra.validate(
            [],
            &mut source_graph(&fixture.source),
            &fixture.foundation,
            &fixture.registrations,
        ),
        Err(ParamFreeShapeSupportValidationError::PlanMismatch)
    ));
    let missing: DecodedParamFreeShapeSupportPlanSetV1 = decode_canonical(b"\x80").unwrap();
    assert!(matches!(
        missing.validate(
            [&fixture.source],
            &mut source_graph(&fixture.source),
            &fixture.foundation,
            &fixture.registrations,
        ),
        Err(ParamFreeShapeSupportValidationError::PlanMismatch)
    ));
}

#[test]
fn all_providers_encode_empty_plans_as_arrays_and_reject_old_core_wrappers() {
    for provider in [ConeIdentity::CORE, ConeIdentity::SINGLE_FILE] {
        let fixture = finish_fixture(
            source_fixture_parts(source(provider, SourceNominalKind::Interface)),
            None,
        );
        let plan = ParamFreeShapeSupportPlanSetV1::from_sources(
            [],
            &fixture.foundation,
            &fixture.registrations,
        )
        .unwrap();
        assert_eq!(encode(&plan).unwrap(), b"\x80");
        let decoded: DecodedParamFreeShapeSupportPlanSetV1 = decode_canonical(b"\x80").unwrap();
        assert_eq!(
            decoded
                .validate(
                    [],
                    &mut source_graph(&fixture.source),
                    &fixture.foundation,
                    &fixture.registrations,
                )
                .unwrap(),
            plan
        );
    }
    for old_wrapper in [b"\xa1\x00\x01".as_slice(), b"\xa2\x00\x02\x01\x80"] {
        assert!(decode_canonical::<DecodedParamFreeShapeSupportPlanSetV1>(old_wrapper,).is_err());
    }
}
