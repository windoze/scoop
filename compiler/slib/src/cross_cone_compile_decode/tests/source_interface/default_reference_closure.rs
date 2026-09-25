use super::*;
use crate::CrossConeHirDefaultProviderContractError as ContractError;
use scoop_hir::{
    ExportDefaultReferenceClosureValidationError as ClosureError, ExportDefaultReferenceKindV1,
    ExportDefaultReferenceOccurrenceSiteV1, ExportDefaultReferenceSetV1, ExportDefaultReferenceV1,
};

#[test]
fn ordinary_reader_rejects_a_missing_body_reference_at_its_exact_occurrence() {
    let mut fixture = default_fixture::fixture(default_fixture::Case::Defined);
    let origin = fixture.interface.default_templates().records()[0]
        .definition_origin()
        .clone();
    replace_types(&mut fixture, vec![]);
    let ClosureError::Missing {
        kind,
        site,
        definition_origin,
        ..
    } = failure(&fixture)
    else {
        panic!("a body reference must have its declaration-side record")
    };
    assert_eq!(kind, ExportDefaultReferenceKindV1::Type);
    assert_eq!(
        site,
        ExportDefaultReferenceOccurrenceSiteV1::TemplateLocalType { index: 0 }
    );
    assert_eq!(*definition_origin, origin);
}

#[test]
fn ordinary_reader_rejects_an_unused_reference_even_with_a_valid_public_witness() {
    let mut fixture = default_fixture::fixture(default_fixture::Case::Defined);
    let mut references = fixture.interface.default_templates().records()[0]
        .references()
        .types()
        .to_vec();
    let reference = &references[0];
    references.push(ExportDefaultReferenceV1::new(
        SignatureTypeKey::Tuple(scoop_identity::NonEmptyVec::from_first(
            reference.target().clone(),
            [],
        )),
        reference.definition_origin().clone(),
        reference.witness().clone(),
    ));
    references.sort_unstable();
    let extra_index = references
        .iter()
        .position(|reference| matches!(reference.target(), SignatureTypeKey::Tuple(_)))
        .unwrap();
    replace_types(&mut fixture, references);
    assert_eq!(
        failure(&fixture),
        ClosureError::Extra {
            kind: ExportDefaultReferenceKindV1::Type,
            index: extra_index,
        }
    );
}

#[test]
fn ordinary_reader_matches_reference_origins_in_addition_to_typed_targets() {
    let mut fixture = default_fixture::fixture(default_fixture::Case::Defined);
    let reference = &fixture.interface.default_templates().records()[0]
        .references()
        .types()[0];
    let original = reference.definition_origin().clone();
    let source = original.origin().source().clone();
    let changed = ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(
            source.clone(),
            SourceSpan::new(6, 13).unwrap(),
            &SourceContextKey::File { source },
        )
        .unwrap(),
    );
    let references = vec![ExportDefaultReferenceV1::new(
        reference.target().clone(),
        changed,
        reference.witness().clone(),
    )];
    replace_types(&mut fixture, references);
    assert!(matches!(failure(&fixture), ClosureError::Missing {
        kind: ExportDefaultReferenceKindV1::Type, definition_origin, ..
    } if *definition_origin == original));
}

fn failure(fixture: &CallableSourceSurface) -> ClosureError {
    let bytes = fixture.artifact();
    let Err(CrossConeHirSourceInterfaceSurfaceError::DefaultProviderContract(
        ContractError::Template { index, key, source },
    )) = validate_until_type_alias(&bytes).validate_source_interfaces(vec![])
    else {
        panic!("an inexact body-reference closure must fail ordinary artifact validation")
    };
    assert_eq!(index, 0);
    assert_eq!(key.owner(), fixture.owner);
    assert_eq!(key.parameter_position(), 0);
    let ContractError::ReferenceClosure(error) = *source else {
        panic!("reference closure validation")
    };
    *error
}

fn replace_types(
    fixture: &mut CallableSourceSurface,
    types: Vec<ExportDefaultReferenceV1<SignatureTypeKey>>,
) {
    let references = fixture.interface.default_templates().records()[0].references();
    let references = ExportDefaultReferenceSetV1::try_new(
        references.callables().to_vec(),
        references.constructors().to_vec(),
        types,
        references.globals().to_vec(),
        references.singleton_values().to_vec(),
        references.fields().to_vec(),
    )
    .unwrap();
    default_fixture::replace_references(fixture, references);
}
