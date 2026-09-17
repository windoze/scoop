use scoop_identity::{
    CanonicalIdentifier, ConeCoordinate, ConeIdentity, CoreBuiltinNominal, DeclarationScope,
    DefinitionOwnerChain, DependencyCallableDeclarationId, Effect, ExactCallableSignature,
    ExactTypeKey, PackagePath, PersistentExactTypeId, PersistentFunctionId, SourceDeclarationKey,
    SourceDeclarationSite, StrongCallableDefinitionOwner,
};

use super::*;
use crate::{CanonicalMirFoundation, OdrFreeMirFoundation};

#[test]
fn selected_dependency_set_preserves_canonical_bridge_order_and_typed_lookup() {
    let consumer = cone("consumer");
    let first_provider = cone("a-provider");
    let second_provider = cone("z-provider");
    let first = selected(first_provider, "first");
    let second = selected(second_provider, "second");
    let bridge = bridge(consumer, vec![second.clone(), first.clone()]);

    let selected = SelectedDependencyMirSet::try_from_bridge(&bridge).unwrap();
    let mut expected = vec![first.clone(), second.clone()];
    expected.sort_unstable_by_key(|record| (record.provider(), record.declaration()));

    assert_eq!(selected.consumer(), consumer);
    assert_eq!(selected.callables(), expected);
    assert_eq!(selected.len(), 2);
    assert!(!selected.is_empty());

    let first_id = selected
        .callable_for(first.provider(), first.declaration())
        .unwrap();
    assert_eq!(selected.callable(first_id), Some(&first));
    let reference = selected.callable_ref(first_id).unwrap();
    assert_eq!(reference.callable(), first_id);
    assert_eq!(selected.resolve_callable(reference), Some(&first));
    let managed = selected
        .callable_use(first_id, crate::GcEffect::Managed)
        .unwrap();
    let no_gc = selected
        .callable_use(first_id, crate::GcEffect::NoGc)
        .unwrap();
    assert_eq!(managed.reference(), reference);
    assert_eq!(managed.gc_effect(), crate::GcEffect::Managed);
    assert_eq!(no_gc.reference(), reference);
    assert_eq!(no_gc.gc_effect(), crate::GcEffect::NoGc);
}

#[test]
fn equal_dependency_selections_have_distinct_request_local_brands() {
    let consumer = cone("consumer");
    let provider = cone("provider");
    let callable = selected(provider, "run");
    let bridge = bridge(consumer, vec![callable.clone()]);
    let first = SelectedDependencyMirSet::try_from_bridge(&bridge).unwrap();
    let second = SelectedDependencyMirSet::try_from_bridge(&bridge).unwrap();
    let first_id = first
        .callable_for(callable.provider(), callable.declaration())
        .unwrap();
    let second_id = second
        .callable_for(callable.provider(), callable.declaration())
        .unwrap();

    assert_eq!(first_id, second_id);
    assert!(
        second
            .resolve_callable(first.callable_ref(first_id).unwrap())
            .is_none()
    );
}

#[test]
fn empty_dependency_selection_retains_its_consumer_authority() {
    let consumer = cone("consumer");
    let bridge = bridge(consumer, Vec::new());
    let selected = SelectedDependencyMirSet::try_from_bridge(&bridge).unwrap();

    assert_eq!(selected.consumer(), consumer);
    assert!(selected.is_empty());
    assert!(selected.callables().is_empty());
}

fn bridge(
    consumer: ConeIdentity,
    selected: Vec<SelectedDependencyMirCallableV1>,
) -> CrossConeMirBridgeSectionV1 {
    let foundation = OdrFreeMirFoundation::try_new(CanonicalMirFoundation::empty()).unwrap();
    CrossConeMirBridgeSectionV1::try_new(consumer, &foundation, Vec::new(), selected).unwrap()
}

fn selected(provider: ConeIdentity, name: &str) -> SelectedDependencyMirCallableV1 {
    let declaration = source_function(provider, name);
    let function = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
    SelectedDependencyMirCallableV1::try_new(
        provider,
        DependencyCallableDeclarationId::Function(function),
        StrongCallableDefinitionOwner::Function(function),
        ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), core_unit()),
    )
    .unwrap()
}

fn source_function(cone: ConeIdentity, name: &str) -> SourceDeclarationKey {
    SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            cone,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    )
}

fn core_unit() -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .unwrap()
}

fn cone(name: &str) -> ConeIdentity {
    ConeCoordinate::new("tests", name, "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}
