use super::*;
use crate::*;
use scoop_identity::*;
use scoop_wire::{BudgetMeter, DecodeLimits, decode_canonical, encode};

mod layout;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

#[test]
fn shape_link_subject_contract_matrix_is_closed() {
    assert_eq!(
        subjects().map(super::contract::contract_tag),
        [1, 2, 3, 4, 5, 4, 6, 6, 7, 7]
    );
}

fn subjects() -> [ExternalStrongShapeSubjectV1; 10] {
    let bound = crate::exact_layout::tests::Bound::value(crate::exact_layout::tests::exact(
        &crate::exact_layout::tests::source("Value", SourceNominalKind::Struct, 0),
    ));
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let function = PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        site.clone(),
        CanonicalIdentifier::new("call").unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap();
    let property = PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        site,
        CanonicalIdentifier::new("value").unwrap(),
    ))
    .unwrap();
    let storage = StaticStorageIdentity::property_backing(
        PropertyOwner::Property(property),
        MaterializationRoot::cone_owned(),
    )
    .unwrap()
    .identity_record()
    .id();
    let unit = PersistentInitializationUnitId::from_key(&InitializationUnitKey::TopLevelProperty(
        property,
    ))
    .unwrap();
    let exact = bound.identity.exact();
    use ExternalStrongShapeSubjectV1 as S;
    [
        S::Callable(StrongCallableDefinitionOwner::Function(function)),
        S::Layout(bound.identity.layout()),
        S::Scan(bound.foundation.scans()[0].id()),
        S::TypeDescriptor(exact),
        S::DispatchTable(
            PersistentDispatchTableId::from_key(&DispatchTableKey::vtable(exact)).unwrap(),
        ),
        S::TypeRegistration(exact),
        S::StaticStorage(storage),
        S::StaticStorageRegistration(storage),
        S::InitializationCell(unit),
        S::InitializationDescriptor(unit),
    ]
}

#[test]
fn no_shape_support_never_grants_object_or_initialization_relations() {
    for (index, subject) in subjects().into_iter().enumerate() {
        let result =
            NoShapeLinkSupportV1.support_source(ConeIdentity::SINGLE_FILE, subject, &mut meter());
        if index < 6 {
            assert!(result.unwrap().is_none());
        } else {
            assert!(
                matches!(result, Err(ShapeLinkError::SupportRelation(actual)) if actual == subject)
            );
        }
    }
}

#[test]
fn shape_link_contract_reader_rejects_unknown_tags_and_wrong_product_arity() {
    for bytes in [
        vec![0xa2, 0, 0, 1, 0],
        vec![0xa2, 0, 8, 1, 0],
        vec![0xa1, 0, 2],
        vec![0xa2, 0, 1, 1, 0],
        vec![0xa2, 0, 3, 1, 0],
        vec![0xa4, 0, 4],
    ] {
        assert!(
            decode_canonical::<DecodedShapeLinkContractV1>(&bytes, DecodeLimits::default())
                .is_err()
        );
    }
}
