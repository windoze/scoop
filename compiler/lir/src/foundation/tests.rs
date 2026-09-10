use scoop_identity::{
    CallableBodyKey, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionOwnerChain, ExactTypeKey, LayoutKey, PackagePath, PersistentCallableBodyId,
    PersistentExactTypeId, PersistentFunctionId, PersistentTypeId, RuntimeIdentityRecord,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind, StrongCallableDefinitionOwner,
};
use scoop_wire::encode;

use super::{CanonicalLirFoundation, LirFoundationBuildError, LirFoundationTable};
use crate::RuntimeTypeMappingRecord;

#[test]
fn empty_foundation_has_all_twenty_two_empty_tables() {
    let actual = encode(&CanonicalLirFoundation::empty()).unwrap();
    let mut expected = vec![0xb6];
    for field in 1_u8..=22 {
        expected.push(field);
        expected.push(0x80);
    }
    assert_eq!(actual, expected);
}

#[test]
fn exact_types_use_dependency_first_order_instead_of_raw_id_order() {
    let (child, parent) = dependency_pair_with_parent_sorting_first();
    assert!(parent.id() < child.id());

    let mut foundation = CanonicalLirFoundation::empty();
    foundation
        .set_exact_types(vec![parent.clone(), child.clone()])
        .unwrap();

    assert_eq!(foundation.exact_types, vec![child, parent]);
}

#[test]
fn exact_type_dependencies_from_earlier_layers_are_not_local_ordering_errors() {
    let (external_child, _) = nominal_exact("ExternalChild");
    let local_parent =
        CborIdentityRecord::from_key(ExactTypeKey::RawPointer(external_child)).unwrap();
    let mut foundation = CanonicalLirFoundation::empty();

    foundation
        .set_exact_types(vec![local_parent.clone()])
        .unwrap();

    assert_eq!(foundation.exact_types, vec![local_parent]);
}

#[test]
fn simple_and_runtime_mapping_tables_are_canonicalized_by_primary_id() {
    let (first_exact, _) = nominal_exact("First");
    let (second_exact, _) = nominal_exact("Second");
    let first_layout = CborIdentityRecord::from_key(LayoutKey::darwin_aarch64(
        first_exact,
        scoop_identity::RepresentationRole::ManagedObject,
    ))
    .unwrap();
    let second_layout = CborIdentityRecord::from_key(LayoutKey::darwin_aarch64(
        second_exact,
        scoop_identity::RepresentationRole::ManagedObject,
    ))
    .unwrap();
    let first_mapping = RuntimeTypeMappingRecord::new(first_exact).unwrap();
    let second_mapping = RuntimeTypeMappingRecord::new(second_exact).unwrap();

    let mut foundation = CanonicalLirFoundation::empty();
    foundation
        .set_layouts(vec![second_layout.clone(), first_layout.clone()])
        .unwrap();
    foundation
        .set_runtime_types(vec![second_mapping, first_mapping])
        .unwrap();

    assert!(foundation.layouts[0].id() < foundation.layouts[1].id());
    assert!(foundation.runtime_types[0].exact_type() < foundation.runtime_types[1].exact_type());

    let mut other = CanonicalLirFoundation::empty();
    other
        .set_layouts(vec![first_layout, second_layout])
        .unwrap();
    other
        .set_runtime_types(vec![first_mapping, second_mapping])
        .unwrap();
    assert_eq!(encode(&foundation).unwrap(), encode(&other).unwrap());
}

#[test]
fn callable_bodies_without_dependencies_are_sorted_by_persistent_id() {
    let first = callable_body("first");
    let second = callable_body("second");
    let mut foundation = CanonicalLirFoundation::empty();

    foundation.set_callable_bodies(vec![second, first]).unwrap();

    assert!(foundation.callable_bodies[0].id() < foundation.callable_bodies[1].id());
}

#[test]
fn duplicate_primary_identity_is_rejected_before_encoding() {
    let (exact, _) = nominal_exact("Duplicate");
    let layout = CborIdentityRecord::from_key(LayoutKey::darwin_aarch64(
        exact,
        scoop_identity::RepresentationRole::ManagedValue,
    ))
    .unwrap();
    let mut foundation = CanonicalLirFoundation::empty();

    let error = foundation
        .set_layouts(vec![layout.clone(), layout])
        .unwrap_err();

    assert!(matches!(
        error,
        LirFoundationBuildError::DuplicateIdentity {
            table: LirFoundationTable::Layout,
            ..
        }
    ));
}

fn dependency_pair_with_parent_sorting_first() -> (
    CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>,
    CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>,
) {
    for ordinal in 0..1_000 {
        let (child_id, child) = nominal_exact(&format!("Dependency{ordinal}"));
        let parent = CborIdentityRecord::from_key(ExactTypeKey::RawPointer(child_id)).unwrap();
        if parent.id() < child.id() {
            return (child, parent);
        }
    }
    panic!("expected to find a deterministic hash pair within the test bound")
}

fn nominal_exact(
    name: &str,
) -> (
    PersistentExactTypeId,
    CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>,
) {
    let declaration = SourceDeclarationKey::nominal(
        declaration_site(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        0,
    );
    let nominal = PersistentTypeId::from_source_declaration(&declaration).unwrap();
    let record = CborIdentityRecord::from_key(ExactTypeKey::Nominal(nominal)).unwrap();
    (record.id(), record)
}

fn callable_body(name: &str) -> RuntimeIdentityRecord<PersistentCallableBodyId> {
    let declaration = SourceDeclarationKey::function(
        declaration_site(),
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    );
    let function = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
    RuntimeIdentityRecord::from_key(&CallableBodyKey::strong(
        StrongCallableDefinitionOwner::Function(function),
    ))
    .unwrap()
}

fn declaration_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}
