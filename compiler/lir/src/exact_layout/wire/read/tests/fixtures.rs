use super::*;

pub(super) fn aggregate() -> ExactLayoutExportV1 {
    let owner = source("Fields", SourceNominalKind::Struct, 0);
    let fields = [field(&owner, "unit"), field(&owner, "reference")];
    let values = [unit(), managed()];
    let inputs: Vec<_> = fields
        .iter()
        .zip(&values)
        .map(|(field, value)| NominalLayoutFieldInputV1 { field, value })
        .collect();
    let bound = Bound::value(exact(&owner));
    ExactValueLayoutV1::ordinary_struct(bound.identity, true, &inputs, &bound.foundation)
        .unwrap()
        .into()
}

pub(super) fn tuple() -> ExactLayoutExportV1 {
    let values = [unit(), managed()];
    let bound = Bound::value(
        CborIdentityRecord::from_key(ExactTypeKey::Tuple(
            NonEmptyVec::new(
                values
                    .iter()
                    .map(|value| value.identity().exact())
                    .collect(),
            )
            .unwrap(),
        ))
        .unwrap(),
    );
    ExactValueLayoutV1::tuple(bound.identity, &[&values[0], &values[1]], &bound.foundation)
        .unwrap()
        .into()
}

pub(super) fn enumeration(niche: bool) -> ExactLayoutExportV1 {
    let owner = source("Enum", SourceNominalKind::Enum, 0);
    let variants: Vec<_> = ["First", "Second"]
        .map(|name| {
            CborIdentityRecord::from_key(
                EnumVariantIdentityKey::source(&owner, CanonicalIdentifier::new(name).unwrap())
                    .unwrap(),
            )
            .unwrap()
        })
        .into();
    let fields: Vec<_> = variants
        .iter()
        .map(|variant| {
            CborIdentityRecord::from_key(EnumVariantFieldKey::new(
                variant.id(),
                EnumVariantFieldSelector::Positional {
                    declaration_index: 0,
                },
            ))
            .unwrap()
        })
        .collect();
    let values = [integer("Long", IntegerKind::SIGNED_64), managed()];
    let field_inputs: Vec<_> = fields
        .iter()
        .zip(&values)
        .map(|(field, value)| [EnumLayoutFieldInputV1 { field, value }])
        .collect();
    let variants = [
        EnumLayoutVariantInputV1 {
            variant: &variants[0],
            fields: if niche { &[] } else { &field_inputs[0] },
        },
        EnumLayoutVariantInputV1 {
            variant: &variants[1],
            fields: &field_inputs[1],
        },
    ];
    let bound = Bound::value(exact(&owner));
    ExactValueLayoutV1::enumeration(bound.identity, &variants, &bound.foundation)
        .unwrap()
        .into()
}

pub(super) fn bytes() -> ExactLayoutExportV1 {
    let bound = Bound::instance(exact(&source("String", SourceNominalKind::Class, 0)));
    ExactInstanceLayoutV1::inline_bytes(bound.identity, &bound.foundation)
        .unwrap()
        .into()
}

pub(super) fn array(zst: bool) -> ExactLayoutExportV1 {
    let element = if zst { unit() } else { managed() };
    let origin = PersistentGenericTypeId::from_source_declaration(&source(
        "Array",
        SourceNominalKind::Class,
        1,
    ))
    .unwrap();
    let key = CborIdentityRecord::from_key(ExactTypeKey::NominalApplication {
        origin,
        arguments: NonEmptyVec::from_first(element.identity().exact(), []),
    })
    .unwrap();
    let bound = Bound::new(
        key,
        RepresentationRole::ManagedObject,
        ScanRole::ArrayElement,
    );
    ExactInstanceLayoutV1::inline_array(bound.identity, &element, &bound.foundation)
        .unwrap()
        .into()
}

pub(super) fn boxed() -> ExactLayoutExportV1 {
    let value = aggregate().value_handle().unwrap();
    let boxed = PersistentTypeId::from_generated_key(&GeneratedNominalKey::BoxedValue {
        payload: value.identity().exact(),
    })
    .unwrap();
    let bound =
        Bound::instance(CborIdentityRecord::from_key(ExactTypeKey::Nominal(boxed)).unwrap());
    ExactInstanceLayoutV1::boxed_payload(bound.identity, &value, &bound.foundation)
        .unwrap()
        .into()
}

pub(super) fn class() -> ExactLayoutExportV1 {
    let bound = Bound::instance(exact(&source("Base", SourceNominalKind::Class, 0)));
    let base = ExactInstanceLayoutV1::class(
        bound.identity,
        ClassLayoutBaseV1::NoBase,
        &[],
        &bound.foundation,
    )
    .unwrap();
    let bound = Bound::instance(exact(&source("Derived", SourceNominalKind::Class, 0)));
    ExactInstanceLayoutV1::class(
        bound.identity,
        ClassLayoutBaseV1::Base(&base),
        &[],
        &bound.foundation,
    )
    .unwrap()
    .into()
}
