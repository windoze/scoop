use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOrigin,
    DefinitionOriginRecord, DefinitionOriginSubject, DefinitionOwnerAtom, DefinitionOwnerChain,
    GeneratedNominalKey, NormalizedSourcePath, PackagePath, PersistentExactTypeId,
    PersistentTypeId, SourceContextKey, SourceDeclarationKey, SourceDeclarationSite,
    SourceIdentity, SourceNominalKind, SourceSpan,
};
use scoop_wire::encode;

use super::{CanonicalHirFoundation, HirFoundationBuildError, HirFoundationTable};
use crate::{NativeBoundaryNominalShape, NativeBoundaryTypeDefinitionRecord, SourceRecord};

#[test]
fn empty_foundation_has_all_thirty_two_empty_tables() {
    let actual = encode(&CanonicalHirFoundation::empty()).unwrap();
    let mut expected = vec![0xb8, 32];
    for field in (1_u8..=29).chain(31..=33) {
        if field < 24 {
            expected.push(field);
        } else {
            expected.extend([0x18, field]);
        }
        expected.push(0x80);
    }
    assert_eq!(actual, expected);
}

#[test]
fn nested_source_declarations_use_dependency_first_order() {
    let (parent, child) = type_dependency_pair_with_child_sorting_first();
    assert!(child.id() < parent.id());
    let mut foundation = CanonicalHirFoundation::empty();

    foundation
        .set_types(vec![child.clone(), parent.clone()])
        .unwrap();

    assert_eq!(foundation.types, vec![parent, child]);
}

#[test]
fn source_records_are_sorted_by_semantic_source_identity() {
    let first = source_record("src/a.scoop");
    let second = source_record("src/b.scoop");
    let mut foundation = CanonicalHirFoundation::empty();

    foundation
        .set_sources(vec![second.clone(), first.clone()])
        .unwrap();

    assert_eq!(foundation.sources, vec![first.clone(), second.clone()]);
    let first_bytes = encode(&foundation).unwrap();
    foundation.set_sources(vec![first, second]).unwrap();
    assert_eq!(encode(&foundation).unwrap(), first_bytes);
}

#[test]
fn current_source_count_excludes_retained_foreign_definitions() {
    let current = SourceIdentity::new(
        ConeIdentity::SINGLE_FILE,
        NormalizedSourcePath::new("main.scoop").unwrap(),
    )
    .unwrap();
    let mut foundation = CanonicalHirFoundation::empty();
    foundation
        .set_sources(vec![
            SourceRecord::from_utf8(current, "", []).unwrap(),
            source_record("src/default.scoop"),
            source_record("src/helper.scoop"),
        ])
        .unwrap();
    assert_eq!(foundation.counts().sources, 3);
    assert_eq!(
        foundation.source_count_for_cone(ConeIdentity::SINGLE_FILE),
        1
    );
    assert_eq!(foundation.source_count_for_cone(ConeIdentity::CORE), 2);
}

#[test]
fn duplicate_identity_and_definition_subject_are_rejected() {
    let exact = nominal_exact("Payload");
    let generated =
        CborIdentityRecord::from_key(GeneratedNominalKey::BoxedValue { payload: exact }).unwrap();
    let mut foundation = CanonicalHirFoundation::empty();
    assert!(matches!(
        foundation.set_generated_types(vec![generated.clone(), generated]),
        Err(HirFoundationBuildError::DuplicateIdentity {
            table: HirFoundationTable::GeneratedType,
            ..
        })
    ));

    let source = source_identity("src/origin.scoop");
    let context_key = SourceContextKey::File {
        source: source.clone(),
    };
    let origin =
        DefinitionOrigin::new(source, SourceSpan::new(0, 0).unwrap(), &context_key).unwrap();
    let subject = DefinitionOriginSubject::Type(source_type("OriginOwner"));
    let record = DefinitionOriginRecord::new(subject, origin);
    assert!(matches!(
        foundation.set_definition_origins(vec![record.clone(), record]),
        Err(HirFoundationBuildError::DuplicateSubject {
            table: HirFoundationTable::DefinitionOrigin,
            ..
        })
    ));
}

#[test]
fn duplicate_native_boundary_owner_is_rejected() {
    let declaration = source_nominal("Boundary", DefinitionOwnerChain::top_level());
    let record = NativeBoundaryTypeDefinitionRecord::new(
        &declaration,
        &[0],
        NativeBoundaryNominalShape::Reference,
    )
    .unwrap();
    let mut foundation = CanonicalHirFoundation::empty();

    assert!(matches!(
        foundation.set_native_boundary_types(vec![record.clone(), record]),
        Err(HirFoundationBuildError::DuplicateSubject {
            table: HirFoundationTable::NativeBoundaryType,
            ..
        })
    ));
}

fn type_dependency_pair_with_child_sorting_first() -> (
    CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
    CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
) {
    let parent_key = source_nominal("Parent", DefinitionOwnerChain::top_level());
    let parent_id = PersistentTypeId::from_source_declaration(&parent_key).unwrap();
    let parent = CborIdentityRecord::from_key(parent_key).unwrap();
    let owners =
        DefinitionOwnerChain::from_outer_to_inner(vec![DefinitionOwnerAtom::Type(parent_id)]);
    for ordinal in 0..1_000 {
        let child = CborIdentityRecord::from_key(source_nominal(
            &format!("Child{ordinal}"),
            owners.clone(),
        ))
        .unwrap();
        if child.id() < parent_id {
            return (parent, child);
        }
    }
    panic!("expected to find a deterministic hash pair within the test bound")
}

fn source_record(path: &str) -> SourceRecord {
    SourceRecord::from_utf8(source_identity(path), "unit\n", [0, 4]).unwrap()
}

fn source_identity(path: &str) -> SourceIdentity {
    SourceIdentity::new(ConeIdentity::CORE, NormalizedSourcePath::new(path).unwrap()).unwrap()
}

fn nominal_exact(name: &str) -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&scoop_identity::ExactTypeKey::Nominal(source_type(name)))
        .unwrap()
}

fn source_type(name: &str) -> PersistentTypeId {
    PersistentTypeId::from_source_declaration(&source_nominal(
        name,
        DefinitionOwnerChain::top_level(),
    ))
    .unwrap()
}

fn source_nominal(name: &str, owners: DefinitionOwnerChain) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            owners,
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        0,
    )
}

#[test]
fn reader_rejects_retired_native_witness_field_thirty() {
    let mut bytes = vec![0xb8, 32];
    for field in 1_u8..=32 {
        if field < 24 {
            bytes.push(field);
        } else {
            bytes.extend([0x18, field]);
        }
        bytes.push(0x80);
    }
    let error = scoop_wire::decode_canonical::<crate::DecodedHirFoundation>(
        &bytes,
        scoop_wire::DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        &scoop_wire::WireErrorKind::UnexpectedField {
            expected: 31,
            actual: 30
        }
    );
}
