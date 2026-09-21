use std::collections::BTreeMap;

use scoop_hir::{
    CanonicalTypeAliasExpansionsV1, CanonicalTypeAliasInterfacesV1,
    ImportedDependencySelectionPlan, TypeAliasClosureAuthority, TypeAliasInterfaceRecordV1,
};
use scoop_identity::{
    ConeIdentity, CoreBuiltinNominal, PersistentTypeAliasId, SemanticIdentitySession,
    SemanticOriginFingerprint,
};
use scoop_wire::{BudgetMeter, DecodeLimits, WirePath};

use super::{artifact, decode, target};
use crate::{
    CrossConeProviderRole, CrossConeSemanticCommitError, DecodedCrossConeClosure,
    LirBridgeValidatedCrossConeHirClosure, LirBridgeValidatedCrossConeHirFrontSections,
    strong_compile_decode::tests::cone_named,
};

#[test]
fn semantic_commit_accepts_the_valid_empty_core_closure() {
    let closure =
        DecodedCrossConeClosure::new(ConeIdentity::CORE, target(), Vec::new(), Vec::new())
            .validate_profile_graph()
            .unwrap()
            .validate_identities()
            .unwrap()
            .validate_foundation_structure()
            .unwrap()
            .resolve_hir_interfaces()
            .unwrap()
            .validate_hir_productions()
            .unwrap()
            .validate_internal_hir_closures()
            .unwrap()
            .validate_definition_sources()
            .unwrap()
            .validate_nominal_surfaces()
            .unwrap()
            .validate_property_surfaces()
            .unwrap()
            .validate_callable_surfaces()
            .unwrap()
            .validate_type_alias_surfaces()
            .unwrap()
            .validate_source_interfaces()
            .unwrap()
            .validate_const_values()
            .unwrap()
            .validate_public_binding_routes()
            .unwrap()
            .validate_external_hir_references()
            .unwrap()
            .validate_and_expand_type_aliases()
            .unwrap()
            .validate_mir_bridges()
            .unwrap()
            .validate_lir_bridges()
            .unwrap();
    let mut session = SemanticIdentitySession::new();
    let committed = closure.commit(&mut session).unwrap();

    assert_eq!(committed.current(), ConeIdentity::CORE);
    assert_eq!(committed.provider_count(), 0);
    assert_eq!(session.origin_count(), 0);
    assert_eq!(session.entity_count(), 0);
    let world = committed.imported_semantic_world().unwrap();
    assert_eq!(world.current(), ConeIdentity::CORE);
    assert_eq!(world.provider_count(), 0);
    assert!(world.direct_provider(ConeIdentity::CORE).is_none());

    let hir_selection = ImportedDependencySelectionPlan::empty(ConeIdentity::CORE).finish();
    let mir_selection = committed
        .project_dependency_callables_to_mir(&hir_selection)
        .unwrap();
    assert_eq!(mir_selection.consumer(), ConeIdentity::CORE);
    assert!(mir_selection.is_empty());
    let lir_selection = committed
        .project_dependency_callables_to_lir(&mir_selection)
        .unwrap();
    assert_eq!(lir_selection.consumer(), ConeIdentity::CORE);
    assert!(lir_selection.is_empty());

    let foreign_hir = ImportedDependencySelectionPlan::empty(ConeIdentity::SINGLE_FILE).finish();
    assert!(matches!(
        committed.project_dependency_callables_to_mir(&foreign_hir),
        Err(crate::CrossConeMirSelectionProjectionError::ConsumerMismatch { .. })
    ));
    let foreign_mir = scoop_mir::SelectedDependencyMirSet::empty(ConeIdentity::SINGLE_FILE);
    assert!(matches!(
        committed.project_dependency_callables_to_lir(&foreign_mir),
        Err(crate::CrossConeLirSelectionProjectionError::ConsumerMismatch { .. })
    ));
}

#[test]
fn semantic_commit_preserves_direct_and_support_capability_boundaries() {
    let support_bytes = artifact(cone_named("support"), Vec::new());
    let support_record = decode(&support_bytes).dependency_record();
    let direct_bytes = artifact(cone_named("direct"), vec![support_record]);
    let support = validate_local_front(&support_bytes);
    let direct = validate_local_front(&direct_bytes);
    let support_identity = support.identity();
    let direct_identity = direct.identity();
    let closure = closure_from_fronts(support, direct);

    let mut session = SemanticIdentitySession::new();
    let committed = closure.commit(&mut session).unwrap();

    assert_eq!(committed.provider_count(), 2);
    assert_eq!(session.origin_count(), 2);
    assert_eq!(
        committed.role(support_identity),
        Some(CrossConeProviderRole::Support)
    );
    assert_eq!(
        committed.role(direct_identity),
        Some(CrossConeProviderRole::Direct)
    );
    assert!(committed.direct_provider(support_identity).is_none());
    assert!(committed.support_provider(direct_identity).is_none());
    assert_eq!(committed.direct_providers().len(), 1);
    assert_eq!(committed.dependency_count(direct_identity), Some(1));
    assert_eq!(
        committed
            .support_provider(support_identity)
            .unwrap()
            .hir_identity(CoreBuiltinNominal::Unit.identity_record().id())
            .unwrap()
            .persistent(),
        CoreBuiltinNominal::Unit.identity_record().id()
    );
    let direct = committed.direct_provider(direct_identity).unwrap();
    assert_eq!(direct.hir().origin(), direct_identity);
    assert!(
        direct
            .production()
            .hir_interface()
            .public_bindings()
            .records()
            .is_empty()
    );
    let world = committed.imported_semantic_world().unwrap();
    assert_eq!(world.provider_count(), 2);
    assert!(world.direct_provider(direct_identity).is_some());
    assert!(world.support_provider(support_identity).is_some());
}

#[test]
fn completed_commit_retains_current_without_exposing_it_as_a_provider() {
    let support_bytes = artifact(cone_named("support-complete"), Vec::new());
    let support_record = decode(&support_bytes).dependency_record();
    let direct_bytes = artifact(cone_named("direct-complete"), vec![support_record.clone()]);
    let direct_record = decode(&direct_bytes).dependency_record();
    let current_bytes = artifact(cone_named("current-complete"), vec![direct_record.clone()]);
    let support = validate_local_front(&support_bytes);
    let direct = validate_local_front(&direct_bytes);
    let current = validate_local_front(&current_bytes);
    let support_identity = support.identity();
    let direct_identity = direct.identity();
    let current_identity = current.identity();
    let positions = BTreeMap::from([
        (support_identity, 0),
        (direct_identity, 1),
        (current_identity, 2),
    ]);
    let closure = LirBridgeValidatedCrossConeHirClosure {
        current: current_identity,
        target: target(),
        direct: vec![direct_identity],
        dependency_first: vec![support, direct, current],
        positions,
        dependency_positions: vec![Vec::new(), vec![0], vec![1]],
        type_alias_expansions: vec![
            empty_alias_expansions(),
            empty_alias_expansions(),
            empty_alias_expansions(),
        ],
    };

    let mut session = SemanticIdentitySession::new();
    let committed = closure.commit(&mut session).unwrap();

    assert_eq!(committed.provider_count(), 2);
    assert_eq!(committed.role(current_identity), None);
    assert!(committed.direct_provider(current_identity).is_none());
    assert!(committed.support_provider(current_identity).is_none());
    assert_eq!(
        committed.current_artifact().unwrap().identity(),
        current_identity
    );
    assert_eq!(session.origin_count(), 3);
}

#[test]
fn semantic_commit_is_atomic_when_a_later_origin_conflicts() {
    let support_bytes = artifact(cone_named("support-conflict"), Vec::new());
    let support_record = decode(&support_bytes).dependency_record();
    let direct_bytes = artifact(cone_named("direct-conflict"), vec![support_record]);
    let support = validate_local_front(&support_bytes);
    let direct = validate_local_front(&direct_bytes);
    let support_identity = support.identity();
    let direct_identity = direct.identity();

    let mut session = SemanticIdentitySession::new();
    {
        let import = direct.semantic_identity_import();
        session
            .import(
                import.origin(),
                SemanticOriginFingerprint::new([17; 32], [18; 32], [19; 32]),
                import.graph(),
            )
            .unwrap();
    }
    let origins_before = session.origin_count();
    let entities_before = session.entity_count();
    let closure = closure_from_fronts(support, direct);

    assert!(matches!(
        closure.commit(&mut session),
        Err(CrossConeSemanticCommitError::SemanticImport(
            scoop_identity::SemanticIdentityImportError::OriginConflict { origin }
        )) if origin == direct_identity
    ));
    assert_eq!(session.origin_count(), origins_before);
    assert_eq!(session.entity_count(), entities_before);
    assert!(session.origin_fingerprint(support_identity).is_none());
}

fn validate_local_front(bytes: &[u8]) -> LirBridgeValidatedCrossConeHirFrontSections<'_> {
    let mut decoded = decode(bytes);
    let identities = decoded
        .validate_foundation_identities(std::iter::empty())
        .unwrap();
    decoded
        .validate_foundation_structure(identities)
        .unwrap()
        .resolve_hir_interface()
        .unwrap()
        .validate_hir_production()
        .unwrap()
        .validate_internal_hir_closures()
        .unwrap()
        .validate_definition_sources()
        .unwrap()
        .validate_nominal_surface(Vec::new())
        .unwrap()
        .validate_property_surface(Vec::new())
        .unwrap()
        .validate_callable_surface(Vec::new())
        .unwrap()
        .validate_type_alias_surface(Vec::new())
        .unwrap()
        .validate_source_interfaces(Vec::new())
        .unwrap()
        .validate_const_values(Vec::new())
        .unwrap()
        .validate_mir_bridge()
        .unwrap()
        .validate_lir_bridge()
        .unwrap()
}

fn closure_from_fronts<'input>(
    support: LirBridgeValidatedCrossConeHirFrontSections<'input>,
    direct: LirBridgeValidatedCrossConeHirFrontSections<'input>,
) -> LirBridgeValidatedCrossConeHirClosure<'input> {
    let support_identity = support.identity();
    let direct_identity = direct.identity();
    let positions = BTreeMap::from([(support_identity, 0), (direct_identity, 1)]);

    LirBridgeValidatedCrossConeHirClosure {
        current: cone_named("current-commit").identity(),
        target: target(),
        direct: vec![direct_identity],
        dependency_first: vec![support, direct],
        positions,
        dependency_positions: vec![Vec::new(), vec![0]],
        type_alias_expansions: vec![empty_alias_expansions(), empty_alias_expansions()],
    }
}

fn empty_alias_expansions() -> CanonicalTypeAliasExpansionsV1 {
    CanonicalTypeAliasInterfacesV1::try_new(Vec::new())
        .unwrap()
        .expand_alias_closure(
            &EmptyAliasAuthority,
            &mut BudgetMeter::new(DecodeLimits::default()),
            &WirePath::root(),
        )
        .unwrap()
}

struct EmptyAliasAuthority;

impl TypeAliasClosureAuthority for EmptyAliasAuthority {
    fn external_type_alias(
        &self,
        _alias: PersistentTypeAliasId,
    ) -> Option<&TypeAliasInterfaceRecordV1> {
        None
    }

    fn is_type_alias_edge_authorized(
        &self,
        _source: PersistentTypeAliasId,
        _target: PersistentTypeAliasId,
    ) -> bool {
        false
    }
}
