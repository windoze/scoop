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
fn empty_foundation_has_all_tables_and_c_runtime_mode() {
    let actual = encode(&CanonicalLirFoundation::empty()).unwrap();
    let mut expected = vec![0xb7];
    for field in 1_u8..=22 {
        expected.push(field);
        expected.push(0x80);
    }
    expected.extend([23, 0]);
    assert_eq!(actual, expected);
}

#[test]
fn native_cxx_mode_survives_wire_and_all_target_requirement_projections() {
    use crate::{CanonicalNativeExternalRequirementSurfaceV1, ConeLirFoundation, LirTargetProfile};
    for cxx in [false, true] {
        let foundation =
            ConeLirFoundation::try_new(ConeIdentity::CORE, CanonicalLirFoundation::empty())
                .unwrap()
                .with_native_cxx(cxx);
        let bytes = encode(foundation.as_canonical()).unwrap();
        let decoded = scoop_wire::decode_canonical::<crate::DecodedLirFoundation>(&bytes).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        for target in [
            LirTargetProfile::DARWIN_AARCH64,
            LirTargetProfile::LINUX_X86_64_GNU,
            LirTargetProfile::LINUX_X86_64_MUSL,
        ] {
            let requirements =
                CanonicalNativeExternalRequirementSurfaceV1::from_foundation(target, &foundation)
                    .unwrap();
            assert_eq!(requirements.cxx(), cxx);
        }
        let mut invalid = bytes;
        *invalid.last_mut().unwrap() = 2;
        assert!(scoop_wire::decode_canonical::<crate::DecodedLirFoundation>(&invalid).is_err());
    }
}

#[test]
fn materialized_exact_type_references_are_sorted_by_persistent_id() {
    let (first, _) = nominal_exact("FirstMaterialized");
    let (second, _) = nominal_exact("SecondMaterialized");
    let mut foundation = CanonicalLirFoundation::empty();

    foundation
        .set_materialized_exact_types(vec![second, first])
        .unwrap();

    assert_eq!(
        foundation.materialized_exact_types,
        if first < second {
            vec![first, second]
        } else {
            vec![second, first]
        }
    );
}

#[test]
fn duplicate_materialized_exact_type_reference_is_rejected() {
    let (exact, _) = nominal_exact("DuplicateMaterialized");
    let mut foundation = CanonicalLirFoundation::empty();

    let error = foundation
        .set_materialized_exact_types(vec![exact, exact])
        .unwrap_err();

    assert!(matches!(
        error,
        LirFoundationBuildError::DuplicateIdentity {
            table: LirFoundationTable::MaterializedExactType,
            ..
        }
    ));
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
