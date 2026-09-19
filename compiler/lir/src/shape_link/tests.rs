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
    let subjects = [
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
    ];
    assert_eq!(
        subjects.map(super::contract::contract_tag),
        [1, 2, 3, 4, 5, 4, 6, 6, 7, 7]
    );
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
