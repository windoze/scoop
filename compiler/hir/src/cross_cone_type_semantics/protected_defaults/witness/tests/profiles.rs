use super::*;
use crate::cross_cone_type_semantics::inheritance::tests::support::site;
use scoop_identity::{CanonicalIdentifier, SourceDeclarationKey, SourceNominalKind};

fn source_only_owner(
    fixture: &mut Fixture,
    owners: &[SourceNominalId],
    name: &str,
    arity: u32,
) -> SourceNominalId {
    let key = SourceDeclarationKey::nominal(
        site(owners),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        arity,
    );
    let id = SourceNominalId::from_source_declaration(&key).unwrap();
    let origin = fixture.graph.origins[&fixture.unit.source].clone();
    fixture.graph.keys.insert(id, key);
    fixture.graph.access.insert(
        id,
        DeclarationAccessSourceV1::try_new(
            DeclaredVisibilityV1::Public,
            owners.to_vec(),
            origin.clone(),
        )
        .unwrap(),
    );
    fixture.graph.origins.insert(id, origin);
    id
}
fn callable(
    fixture: &mut Fixture,
    owners: Vec<SourceNominalId>,
    generic: bool,
) -> ProtectedCallableInterfaceV1 {
    let owner = *owners.last().unwrap();
    let key = SourceDeclarationKey::function(
        site(&owners),
        CanonicalIdentifier::new("method").unwrap(),
        u32::from(generic),
        None,
        vec![],
    );
    let declaration = if generic {
        CallableTemplateOrigin::GenericFunction(
            scoop_identity::PersistentGenericFunctionId::from_source_declaration(&key).unwrap(),
        )
    } else {
        CallableTemplateOrigin::Function(
            scoop_identity::PersistentFunctionId::from_source_declaration(&key).unwrap(),
        )
    };
    fixture.declarations.insert(declaration, key);
    let unit = fixture.unit;
    let effects = fixture
        .payload(
            unit,
            declaration,
            vec![],
            SignatureTypeKey::Nominal(nominal(unit)),
        )
        .effects();
    let binders = if generic {
        vec![TypeParameterBinderV1::new(
            CanonicalIdentifier::new("T").unwrap(),
            TypeParameterBoundsV1::Unconstrained,
        )]
    } else {
        vec![]
    };
    ProtectedCallableInterfaceV1::try_new(
        declaration,
        DeclarationAccessSourceV1::try_new(
            DeclaredVisibilityV1::Protected,
            owners,
            fixture.graph.origins[&owner].clone(),
        )
        .unwrap(),
        ProtectedCallablePayloadV1::try_new(
            declaration,
            owner,
            CanonicalBinderListV1::try_new(binders).unwrap(),
            CanonicalSourceParameterShapesV1::try_new(vec![]).unwrap(),
            SignatureTypeKey::Nominal(nominal(unit)),
            effects,
            CallableModalityV1::Final,
            CanonicalProtectedSlotRefsV1::try_new(vec![]).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn source_profile_uses_complete_owner_chain_and_independent_default_classification() {
    for nested in [false, true] {
        let mut fixture = Fixture::default();
        let outer = source_only_owner(&mut fixture, &[], "Outer", 1);
        let owners = if nested {
            let inner = source_only_owner(&mut fixture, &[outer], "Inner", 0);
            vec![outer, inner]
        } else {
            vec![outer]
        };
        let record = callable(&mut fixture, owners, false);
        let key = ProtectedDefaultTemplateKeyV1::try_new(record.declaration(), 0).unwrap();
        let graph_source = fixture.graph.clone();
        let graph = CheckedNominalInheritanceGraphV1::validate_with_source_roots(
            graph_source.records.values(),
            graph_source.keys.keys().copied(),
            &graph_source,
            &mut meter(),
        )
        .unwrap();
        let source = ProtectedDefaultOwnerSourceV1::Protected(
            record
                .validate_source(&graph, &mut fixture, &mut meter())
                .unwrap(),
        );
        let witness =
            ProtectedDefaultAccessWitnessV1::generic_source_metadata(key.owner()).unwrap();
        let metadata = ExpectedProfile {
            key,
            profile: ProtectedDefaultWitnessSourceProfileV1::GenericSourceMetadata,
        };
        assert!(matches!(
            witness
                .validate_source_profile(key, source, &metadata, &mut meter())
                .unwrap(),
            CheckedProtectedDefaultWitnessSourceV1::GenericSourceMetadata(_)
        ));
        let ordinary = ExpectedProfile {
            key,
            profile: ProtectedDefaultWitnessSourceProfileV1::ParamFree,
        };
        assert!(matches!(
            witness.validate_source_profile(key, source, &ordinary, &mut meter()),
            Err(ProtectedDefaultWitnessSourceError::SourceProfile)
        ));
        let concrete = param_free(key.owner());
        assert_eq!(
            concrete
                .validate_source_profile(key, source, &ordinary, &mut meter())
                .is_ok(),
            nested
        );
        let wrong = ProtectedDefaultTemplateKeyV1::try_new(
            CallableTemplateOrigin::Constructor(
                scoop_identity::PersistentConstructorId::from_source_declaration(
                    &SourceDeclarationKey::constructor(site(&[outer]), vec![]),
                )
                .unwrap(),
            ),
            0,
        )
        .unwrap();
        assert!(matches!(
            witness.validate_source_profile(wrong, source, &metadata, &mut meter()),
            Err(ProtectedDefaultWitnessSourceError::Owner)
        ));
    }
}

#[test]
fn callable_binders_do_not_allow_metadata_downgrade_of_param_free_owner() {
    let mut fixture = Fixture::default();
    let owner = fixture.class("Owner");
    let record = callable(&mut fixture, vec![owner.source], true);
    let key = ProtectedDefaultTemplateKeyV1::try_new(record.declaration(), 0).unwrap();
    let graph_source = fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        graph_source.records.values(),
        &graph_source,
        &mut meter(),
    )
    .unwrap();
    let source = ProtectedDefaultOwnerSourceV1::Protected(
        record
            .validate_source(&graph, &mut fixture, &mut meter())
            .unwrap(),
    );
    let ordinary = ExpectedProfile {
        key,
        profile: ProtectedDefaultWitnessSourceProfileV1::ParamFree,
    };
    assert!(matches!(
        param_free(key.owner())
            .validate_source_profile(key, source, &ordinary, &mut meter())
            .unwrap(),
        CheckedProtectedDefaultWitnessSourceV1::ParamFree(_)
    ));
    let invalid = ExpectedProfile {
        key,
        profile: ProtectedDefaultWitnessSourceProfileV1::GenericSourceMetadata,
    };
    assert!(matches!(
        ProtectedDefaultAccessWitnessV1::generic_source_metadata(key.owner())
            .unwrap()
            .validate_source_profile(key, source, &invalid, &mut meter()),
        Err(ProtectedDefaultWitnessSourceError::SourceProfile)
    ));
}
