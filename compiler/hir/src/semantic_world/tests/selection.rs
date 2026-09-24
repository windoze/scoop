use scoop_identity::{
    BindingNamespace, ConeCoordinate, CoreBuiltinNominal, ExactTypeKey, PersistentExactTypeId,
    SemanticIdentitySession, SignatureTypeKey,
};

use super::super::*;
use super::fixture::{
    CallableProviderFixture, ProviderFixture, certificate, coordinate, empty_alias_expansions,
    identifiers, import_foundation, package,
};

mod nominals;

#[test]
fn selection_owns_callable_and_route_proofs_after_world_views_are_gone() {
    let core = ProviderFixture::empty(ConeCoordinate::reserved_core());
    let unit = CoreBuiltinNominal::Unit.identity_record().id();
    let provider = CallableProviderFixture::new(
        coordinate("callable-provider"),
        package(&["demo"]),
        "run",
        SignatureTypeKey::Nominal(unit),
    );
    let mut session = SemanticIdentitySession::new();
    let core_foundation = import_foundation(&mut session, &core, 41);
    let provider_foundation = import_callable_foundation(&mut session, &provider, 42);
    let aliases = empty_alias_expansions();
    let world = ImportedSemanticWorld::from_validated_closure(
        coordinate("selection-current").identity().unwrap(),
        vec![
            DirectImportedProviderInput::from_validated(
                certificate(&core.coordinate, 41),
                &core_foundation,
                &core.interface,
                &aliases,
            ),
            DirectImportedProviderInput::from_validated(
                certificate(&provider.coordinate, 42),
                &provider_foundation,
                &provider.interface,
                &aliases,
            ),
        ],
        Vec::new(),
    )
    .unwrap();
    let plan = world.dependency_selection_plan().unwrap();
    let binding = world
        .resolve_direct_exact(&identifiers(&["demo", "run"]), BindingNamespace::Value)
        .unwrap()
        .targets()
        .next()
        .unwrap()
        .clone();
    let candidate = plan.callable_candidate(&binding).unwrap();

    assert_eq!(candidate.provider(), provider.identity());
    assert_eq!(candidate.binding().source_count(), 1);
    assert_eq!(
        candidate
            .source_interface()
            .expect("ordinary function has a source interface")
            .owner(),
        scoop_identity::CallableTemplateOrigin::Function(provider.function)
    );
    assert_eq!(
        candidate.capability().unwrap().signature().result(),
        exact(unit)
    );

    let mut winning = plan.clone();
    let reference = winning.select_callable(candidate).unwrap();
    assert_eq!(plan.selected_callable_count(), 0);
    assert_eq!(winning.selected_callable_count(), 1);
    assert_eq!(
        winning
            .resolve_callable(reference)
            .expect("the open transaction resolves its committed reference")
            .provider(),
        provider.identity()
    );
    let selected = winning.finish();
    let callable = selected.resolve_callable(reference).unwrap();
    assert_eq!(selected.consumer(), world.current());
    assert_eq!(callable.provider(), provider.identity());
    assert_eq!(
        callable.capability().declaration().implementation(),
        callable.capability().implementation()
    );
    assert_eq!(
        callable
            .binding()
            .sources()
            .next()
            .unwrap()
            .witness()
            .route()
            .terminal()
            .binding(),
        provider.binding
    );
}

#[test]
fn selection_rejects_semantic_only_and_foreign_projection_candidates() {
    let core = ProviderFixture::empty(ConeCoordinate::reserved_core());
    let unit = CoreBuiltinNominal::Unit.identity_record().id();
    let provider = CallableProviderFixture::new(
        coordinate("semantic-only-provider"),
        package(&["demo"]),
        "raw",
        SignatureTypeKey::RawPointer(Box::new(SignatureTypeKey::Nominal(unit))),
    );
    let mut session = SemanticIdentitySession::new();
    let core_foundation = import_foundation(&mut session, &core, 43);
    let provider_foundation = import_callable_foundation(&mut session, &provider, 44);
    let aliases = empty_alias_expansions();
    let world = ImportedSemanticWorld::from_validated_closure(
        coordinate("semantic-only-current").identity().unwrap(),
        vec![
            DirectImportedProviderInput::from_validated(
                certificate(&core.coordinate, 43),
                &core_foundation,
                &core.interface,
                &aliases,
            ),
            DirectImportedProviderInput::from_validated(
                certificate(&provider.coordinate, 44),
                &provider_foundation,
                &provider.interface,
                &aliases,
            ),
        ],
        Vec::new(),
    )
    .unwrap();
    let mut first = world.dependency_selection_plan().unwrap();
    let mut second = world.dependency_selection_plan().unwrap();
    let binding = world
        .resolve_direct_exact(&identifiers(&["demo", "raw"]), BindingNamespace::Value)
        .unwrap()
        .targets()
        .next()
        .unwrap()
        .clone();
    let candidate = first.callable_candidate(&binding).unwrap();

    assert!(candidate.capability().is_none());
    assert!(matches!(
        first.select_callable(candidate.clone()),
        Err(ImportedDependencySelectionError::CapabilityUnavailable { .. })
    ));
    assert_eq!(first.selected_callable_count(), 0);
    assert_eq!(
        second.select_callable(candidate),
        Err(ImportedDependencySelectionError::ForeignProjection)
    );
}

#[test]
fn cloned_plans_keep_candidate_ids_stable_across_different_probe_orders() {
    let core = ProviderFixture::empty(ConeCoordinate::reserved_core());
    let unit = CoreBuiltinNominal::Unit.identity_record().id();
    let first_provider = CallableProviderFixture::new(
        coordinate("stable-first-provider"),
        package(&["first"]),
        "run",
        SignatureTypeKey::Nominal(unit),
    );
    let second_provider = CallableProviderFixture::new(
        coordinate("stable-second-provider"),
        package(&["second"]),
        "run",
        SignatureTypeKey::Nominal(unit),
    );
    let mut session = SemanticIdentitySession::new();
    let core_foundation = import_foundation(&mut session, &core, 45);
    let first_foundation = import_callable_foundation(&mut session, &first_provider, 46);
    let second_foundation = import_callable_foundation(&mut session, &second_provider, 47);
    let aliases = empty_alias_expansions();
    let world = ImportedSemanticWorld::from_validated_closure(
        coordinate("stable-current").identity().unwrap(),
        vec![
            DirectImportedProviderInput::from_validated(
                certificate(&core.coordinate, 45),
                &core_foundation,
                &core.interface,
                &aliases,
            ),
            DirectImportedProviderInput::from_validated(
                certificate(&second_provider.coordinate, 47),
                &second_foundation,
                &second_provider.interface,
                &aliases,
            ),
            DirectImportedProviderInput::from_validated(
                certificate(&first_provider.coordinate, 46),
                &first_foundation,
                &first_provider.interface,
                &aliases,
            ),
        ],
        Vec::new(),
    )
    .unwrap();
    let plan = world.dependency_selection_plan().unwrap();
    let first = callable_candidate(&world, &plan, &["first", "run"]);
    let second = callable_candidate(&world, &plan, &["second", "run"]);

    let mut first_branch = plan.clone();
    let first_reference = first_branch.select_callable(first).unwrap();
    let mut second_branch = plan;
    let second_reference = second_branch.select_callable(second).unwrap();

    assert!(
        first_branch
            .finish()
            .resolve_callable(second_reference)
            .is_none()
    );
    assert!(
        second_branch
            .finish()
            .resolve_callable(first_reference)
            .is_none()
    );
}

fn exact(source: scoop_identity::PersistentTypeId) -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(source)).unwrap()
}

fn callable_candidate(
    world: &ImportedSemanticWorld<'_>,
    plan: &ImportedDependencySelectionPlan,
    path: &[&str],
) -> ImportedDependencyCallableCandidate {
    let binding = world
        .resolve_direct_exact(&identifiers(path), BindingNamespace::Value)
        .unwrap()
        .targets()
        .next()
        .unwrap()
        .clone();
    plan.callable_candidate(&binding).unwrap()
}

fn import_callable_foundation(
    session: &mut SemanticIdentitySession,
    fixture: &CallableProviderFixture,
    fingerprint: u8,
) -> crate::ImportedHirFoundation {
    let decoded: crate::DecodedHirFoundation = scoop_wire::decode_canonical(
        &scoop_wire::encode(&fixture.foundation).unwrap(),
        scoop_wire::DecodeLimits::default(),
    )
    .unwrap();
    let mut pending = scoop_identity::PendingIdentityValidation::new();
    pending.register_authority(fixture.identity()).unwrap();
    decoded.register_identities(&mut pending).unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    let identities = pending.finish().unwrap();
    let imported = session
        .import(
            fixture.identity(),
            scoop_identity::SemanticOriginFingerprint::new(
                [fingerprint; 32],
                [fingerprint.wrapping_add(1); 32],
                [fingerprint.wrapping_add(2); 32],
            ),
            &identities,
        )
        .unwrap();
    let (hir, _, _) = imported.into_parts();
    crate::ImportedHirFoundation::from_odr_free(
        crate::OdrFreeHirFoundation::try_new(fixture.foundation.clone()).unwrap(),
        hir,
    )
}
