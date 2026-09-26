use super::*;
use hir::{NominalSupportNestedInterfaceV1, NominalSupportPropertyPayloadV1};

pub(super) fn check(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
) {
    let mut records = checked
        .section()
        .protected_declarations()
        .records()
        .to_vec();
    let root = records
        .iter_mut()
        .find_map(|record| match record {
            ProtectedDeclarationInterfaceV1::NestedNominal(record)
                if !record
                    .payload()
                    .source_interface()
                    .children()
                    .values()
                    .is_empty() =>
            {
                Some(record)
            }
            _ => None,
        })
        .unwrap();
    let source = root.payload().source_interface();
    let mut support = source.source_support().records().to_vec();
    for child in &support {
        let NestedSourceSupportV1::NestedNominal(child) = child else {
            continue;
        };
        if child.payload().source_interface().kind() != hir::PublicNominalKindV1::Class {
            continue;
        }
        let hir::SourceNominalId::Concrete(representation_owner) = child.declaration() else {
            panic!("the nested class is non-generic")
        };
        let inheritance_exact = scoop_identity::PersistentExactTypeId::from_key(
            &scoop_identity::ExactTypeKey::Nominal(representation_owner),
        )
        .unwrap();
        assert!(
            !child
                .payload()
                .source_interface()
                .constructors()
                .values()
                .is_empty()
        );
        assert!(
            checked
                .section()
                .inheritance()
                .get(inheritance_exact)
                .unwrap()
                .constructors()
                .records()
                .is_empty()
        );
        assert!(
            checked
                .representations()
                .table()
                .get(representation_owner)
                .is_some()
        );
    }
    let object = support
        .iter_mut()
        .find_map(|record| match record {
            NestedSourceSupportV1::NestedNominal(record)
                if record.payload().source_interface().kind()
                    == hir::PublicNominalKindV1::Object =>
            {
                Some(record)
            }
            _ => None,
        })
        .unwrap();
    let object_source = object.payload().source_interface();
    let mut properties = object_source.source_support().records().to_vec();
    let property = properties
        .iter_mut()
        .find_map(|record| match record {
            NestedSourceSupportV1::Property(property)
                if matches!(
                    property.payload(),
                    NominalSupportPropertyPayloadV1::Const { .. }
                ) =>
            {
                Some(property)
            }
            _ => None,
        })
        .unwrap();
    let id = property.declaration();
    assert!(
        checked
            .metadata()
            .public
            .property_interfaces()
            .get(scoop_identity::PropertyOwner::Property(id))
            .is_none()
    );
    let NominalSupportPropertyPayloadV1::Const { value } = property.payload() else {
        unreachable!()
    };
    **property = hir::NominalSupportPropertyInterfaceV1::try_new(
        id,
        property.declaration_access().clone(),
        NominalSupportPropertyPayloadV1::Const {
            value: hir::ExportConstValueV1::new(
                id,
                value.value_type().clone(),
                hir::CanonicalConstValueV1::Boolean(hir::CanonicalBooleanV1::False),
                value.definition_origin().clone(),
            ),
        },
    )
    .unwrap();
    let interface = nested_interface(
        object_source,
        object_source.modality(),
        object_source.children().clone(),
        hir::CanonicalNestedSourceSupportV1::try_new(properties).unwrap(),
    );
    **object = NominalSupportNestedInterfaceV1::try_new(
        object.declaration(),
        object.declaration_access().clone(),
        hir::ProtectedNestedNominalPayloadV1::try_new(object.declaration(), interface).unwrap(),
    )
    .unwrap();
    **root = replace_nested(
        root,
        nested_interface(
            source,
            source.modality(),
            source.children().clone(),
            hir::CanonicalNestedSourceSupportV1::try_new(support).unwrap(),
        ),
    );
    assert!(
        matches!(reject(checked, core, records), Error::PropertyContract(actual) if actual == id)
    );
}
