use scoop_identity::{
    CallableOwner, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionOwnerChain, Effect, ExactCallableSignature, GeneratedCallableKey,
    GeneratedNominalKey, LexicalCallableParent, LexicalCallableRole, PackagePath,
    PersistentExactTypeId, PersistentFunctionId, PersistentGeneratedCallableId, PersistentTypeId,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment,
};
use scoop_wire::encode;

use super::{CanonicalMirFoundation, MirFoundationBuildError, MirFoundationTable};
use crate::{CallableSignatureRecord, CallableSignatureSubject};

#[test]
fn empty_foundation_has_all_twelve_empty_tables() {
    let actual = encode(&CanonicalMirFoundation::empty()).unwrap();
    let mut expected = vec![0xac];
    for field in 1_u8..=12 {
        expected.push(field);
        expected.push(0x80);
    }
    assert_eq!(actual, expected);
}

#[test]
fn generated_callables_use_dependency_first_order() {
    let (parent, child) = generated_dependency_pair_with_child_sorting_first();
    assert!(child.id() < parent.id());
    let mut foundation = CanonicalMirFoundation::empty();

    foundation
        .set_generated_callables(vec![child.clone(), parent.clone()])
        .unwrap();

    assert_eq!(foundation.generated_callables, vec![parent, child]);
}

#[test]
fn generated_callable_dependencies_from_hir_are_external_to_the_mir_delta() {
    let (_, child) = generated_dependency_pair_with_child_sorting_first();
    let mut foundation = CanonicalMirFoundation::empty();

    foundation
        .set_generated_callables(vec![child.clone()])
        .unwrap();

    assert_eq!(foundation.generated_callables, vec![child]);
}

#[test]
fn simple_tables_and_signature_subjects_have_canonical_order() {
    let first_exact = nominal_exact("First");
    let second_exact = nominal_exact("Second");
    let first_type = CborIdentityRecord::from_key(GeneratedNominalKey::BoxedValue {
        payload: first_exact,
    })
    .unwrap();
    let second_type = CborIdentityRecord::from_key(GeneratedNominalKey::BoxedValue {
        payload: second_exact,
    })
    .unwrap();
    let signature = ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), first_exact);
    let first_signature = CallableSignatureRecord::new(
        CallableSignatureSubject::strong(CallableOwner::Function(source_function("first"))),
        signature.clone(),
    );
    let second_signature = CallableSignatureRecord::new(
        CallableSignatureSubject::strong(CallableOwner::Function(source_function("second"))),
        signature,
    );
    let mut foundation = CanonicalMirFoundation::empty();

    foundation
        .set_generated_types(vec![second_type.clone(), first_type.clone()])
        .unwrap();
    foundation
        .set_callable_signatures(vec![second_signature.clone(), first_signature.clone()])
        .unwrap();

    assert!(foundation.generated_types[0].id() < foundation.generated_types[1].id());
    assert!(
        foundation.callable_signatures[0]
            .subject()
            .compare_sort_key(foundation.callable_signatures[1].subject())
            .is_lt()
    );

    let mut other = CanonicalMirFoundation::empty();
    other
        .set_generated_types(vec![first_type, second_type])
        .unwrap();
    other
        .set_callable_signatures(vec![first_signature, second_signature])
        .unwrap();
    assert_eq!(encode(&foundation).unwrap(), encode(&other).unwrap());
}

#[test]
fn duplicate_primary_and_signature_subjects_are_rejected() {
    let generated = CborIdentityRecord::from_key(GeneratedNominalKey::BoxedValue {
        payload: nominal_exact("Duplicate"),
    })
    .unwrap();
    let mut foundation = CanonicalMirFoundation::empty();
    assert!(matches!(
        foundation.set_generated_types(vec![generated.clone(), generated]),
        Err(MirFoundationBuildError::DuplicateIdentity {
            table: MirFoundationTable::GeneratedType,
            ..
        })
    ));

    let exact = nominal_exact("Result");
    let subject = CallableSignatureSubject::strong(CallableOwner::Function(source_function("f")));
    let first = CallableSignatureRecord::new(
        subject,
        ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), exact),
    );
    let second = first.clone();
    assert!(matches!(
        foundation.set_callable_signatures(vec![first, second]),
        Err(MirFoundationBuildError::DuplicateCallableSignature { .. })
    ));
}

fn generated_dependency_pair_with_child_sorting_first() -> (
    CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>,
    CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>,
) {
    let parent_key = GeneratedCallableKey::Lexical {
        parent: LexicalCallableParent::function(source_function("parent")),
        role: LexicalCallableRole::LambdaBody,
        path: path(0),
    };
    let parent_id = PersistentGeneratedCallableId::from_key(&parent_key).unwrap();
    let generated_parent = LexicalCallableParent::from_generated_key(&parent_key).unwrap();
    let parent = CborIdentityRecord::from_key(parent_key).unwrap();
    for ordinal in 1..1_000 {
        let child = CborIdentityRecord::from_key(GeneratedCallableKey::Lexical {
            parent: generated_parent,
            role: LexicalCallableRole::AnonymousFunctionBody,
            path: path(ordinal),
        })
        .unwrap();
        if child.id() < parent_id {
            return (parent, child);
        }
    }
    panic!("expected to find a deterministic hash pair within the test bound")
}

fn nominal_exact(name: &str) -> PersistentExactTypeId {
    let declaration = SourceDeclarationKey::nominal(
        declaration_site(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        0,
    );
    let nominal = PersistentTypeId::from_source_declaration(&declaration).unwrap();
    PersistentExactTypeId::from_key(&scoop_identity::ExactTypeKey::Nominal(nominal)).unwrap()
}

fn source_function(name: &str) -> PersistentFunctionId {
    let declaration = SourceDeclarationKey::function(
        declaration_site(),
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    );
    PersistentFunctionId::from_source_declaration(&declaration).unwrap()
}

fn path(ordinal: u32) -> StructuralDefinitionPath {
    StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::Lambda, ordinal),
        [],
    )
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
