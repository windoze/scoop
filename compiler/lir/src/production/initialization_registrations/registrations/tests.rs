use la_arena::Arena;
use scoop_identity::{
    CallableBodyKey, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionAtomRole, DefinitionAtomSubkey, DefinitionOwnerChain, DigestNodeId, DigestNodeKey,
    DigestPatchIntentKey, DigestSemanticFieldRole, ExactTypeKey, GeneratedCallableKey,
    InitializationCallableRole, InitializationUnitKey, LinkageClass, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanKey, PackagePath, PersistentCallableBodyId, PersistentExactTypeId,
    PersistentGeneratedCallableId, PersistentInitializationUnitId, PersistentPropertyId,
    PersistentStaticStorageId, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentSymbolRequestTable, PersistentTypeId, PropertyOwner, RuntimeIdentityRecord,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind, StrongCallableDefinitionOwner,
    StrongDefinitionEntity, StrongDefinitionRole,
};

use super::*;
use crate::{
    CanonicalLirFoundation, Global, GlobalInit, LayoutIdentity, LirStaticInitialState,
    LirTargetProfile, LirType, MaterializationRoot, PointerKind, RefScan, StaticStorageIdentity,
    StrongInitializationUnitSemanticPlanSetV1, StrongInitializationUnitSemanticPlanV1,
    StrongStaticStorageSemanticPlanSetV1,
};

#[test]
fn joins_unit_storage_callable_and_digest_relations() {
    let fixture = Fixture::new(Options::default());

    let plans = fixture.build().unwrap();
    let plan = &plans.registrations()[0];

    assert_eq!(plans.producer(), ConeIdentity::SINGLE_FILE);
    assert_eq!(plan.semantic().unit(), fixture.unit);
    assert_eq!(
        plan.registration_symbol().key(),
        PersistentSymbolKey::InitializationRegistration(fixture.unit)
    );
    assert_eq!(
        plan.cell_symbol().key(),
        PersistentSymbolKey::InitializationCell(fixture.unit)
    );
    assert_ne!(
        plan.registration_definition_plan(),
        plan.cell_definition_plan()
    );
    assert_eq!(plan.storage().storage(), fixture.storage);
    assert_eq!(
        plan.storage().storage_symbol().key(),
        PersistentSymbolKey::StaticStorage(fixture.storage)
    );
    assert_eq!(plan.failure_root().storage(), fixture.failure_root);
    assert_eq!(plan.initializer().body(), fixture.initializer);
    assert_eq!(plan.ensure().body(), fixture.ensure);
    assert_eq!(
        plan.schedule().gateway().unwrap().body(),
        fixture.gateway.unwrap()
    );
    assert!(plan.schedule().gateway_definition_patch().is_some());
    assert_eq!(
        fixture
            .foundation
            .definition_atoms()
            .iter()
            .find(|atom| atom.id() == plan.diagnostic_atom())
            .unwrap()
            .key()
            .role(),
        DefinitionAtomRole::AddressTakenConstant
    );
    assert_eq!(
        node(&fixture.digests, plan.registration_fingerprint_node())
            .direct_inputs()
            .len(),
        3
    );
}

#[test]
fn lazy_unit_has_no_gateway_input_or_patch() {
    let fixture = Fixture::new(Options {
        lazy: true,
        ..Options::default()
    });

    let plans = fixture.build().unwrap();
    let plan = &plans.registrations()[0];

    assert_eq!(
        plan.schedule(),
        &StrongInitializationRegistrationSchedulePlanV1::LazyAccess
    );
    assert_eq!(plan.schedule().gateway(), None);
    assert_eq!(
        node(&fixture.digests, plan.registration_fingerprint_node())
            .direct_inputs()
            .len(),
        2
    );
}

#[test]
fn requires_complete_unit_and_referenced_registration_coverage() {
    assert!(matches!(
        Fixture::new(Options {
            omit_unit_registration: true,
            ..Options::default()
        })
        .build(),
        Err(StrongInitializationUnitRegistrationPlanBuildError::UnitSet {
            expected,
            actual
        }) if expected.len() == 1 && actual.is_empty()
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_storage_registration: true,
            ..Options::default()
        })
        .build(),
        Err(
            StrongInitializationUnitRegistrationPlanBuildError::MissingStaticStorageRegistration(_)
        )
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_callable_registration: true,
            ..Options::default()
        })
        .build(),
        Err(StrongInitializationUnitRegistrationPlanBuildError::MissingCallableRegistration(_))
    ));
}

#[test]
fn requires_cell_registration_and_diagnostic_definition_surfaces() {
    assert!(matches!(
        Fixture::new(Options {
            omit_cell_symbol: true,
            ..Options::default()
        })
        .build(),
        Err(StrongInitializationUnitRegistrationPlanBuildError::MissingSymbol(_))
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_registration_primary: true,
            ..Options::default()
        })
        .build(),
        Err(StrongInitializationUnitRegistrationPlanBuildError::PrimaryAtomSet {
            actual,
            ..
        }) if actual.is_empty()
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_registration_object: true,
            ..Options::default()
        })
        .build(),
        Err(StrongInitializationUnitRegistrationPlanBuildError::MissingDigestNode(_))
    ));
    assert!(matches!(
        Fixture::new(Options {
            cell_object_input: true,
            ..Options::default()
        })
        .build(),
        Err(
            StrongInitializationUnitRegistrationPlanBuildError::ObjectLeafInputs {
                leaf: InitializationObjectLeafV1::Cell,
                ..
            }
        )
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_diagnostic_atom: true,
            ..Options::default()
        })
        .build(),
        Err(StrongInitializationUnitRegistrationPlanBuildError::AssociatedAtomSet {
            actual,
            ..
        }) if actual.is_empty()
    ));
}

#[test]
fn requires_exact_inputs_and_schedule_specific_patch_writers() {
    assert!(matches!(
        Fixture::new(Options {
            omit_cell_input: true,
            ..Options::default()
        })
        .build(),
        Err(StrongInitializationUnitRegistrationPlanBuildError::DirectInputs { .. })
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_registration_patch: true,
            ..Options::default()
        })
        .build(),
        Err(StrongInitializationUnitRegistrationPlanBuildError::PatchSet { .. })
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_gateway_patch: true,
            ..Options::default()
        })
        .build(),
        Err(StrongInitializationUnitRegistrationPlanBuildError::GatewayPatchSet { .. })
    ));
    assert!(matches!(
        Fixture::new(Options {
            lazy: true,
            unexpected_lazy_gateway_patch: true,
            ..Options::default()
        })
        .build(),
        Err(StrongInitializationUnitRegistrationPlanBuildError::GatewayPatchSet { .. })
    ));
}

mod fixture;
mod fixture_digest;
use fixture::*;

mod dependencies_v2;

mod registration_v2;
mod shape_link;

pub(in crate::production) fn external_dependency_for_layout_join(
    consumer: ConeIdentity,
) -> crate::StrongInitializationDependencyRefV2 {
    let local = Fixture::new(Options::default());
    let provider = Fixture::with_source(Options::default(), ConeIdentity::CORE, "joinProvider");
    let plans = provider.build().unwrap();
    let definition =
        crate::StrongInitializationUnitDefinitionRefV2::from_registrations(&plans, provider.unit)
            .unwrap();

    let catalog =
        crate::StrongInitializationDefinitionCatalogV2::new(consumer, &[definition]).unwrap();
    let dependency =
        scoop_wire::decode_canonical(&scoop_wire::encode(&provider.unit).unwrap()).unwrap();
    catalog
        .resolve(local.unit, &[dependency])
        .unwrap()
        .references()[0]
}
