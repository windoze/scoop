use super::*;

pub(super) fn bind(bound: &mut BoundNominalSourceContractsV1<'_, '_>) -> Result<(), Error> {
    let own = bound.foundation.foundation.as_canonical();

    let mut fields = BTreeMap::<SourceNominalId, BTreeSet<_>>::new();
    let mut variants = BTreeMap::<SourceNominalId, BTreeSet<_>>::new();
    let mut variant_fields = BTreeMap::<PersistentEnumVariantId, BTreeSet<_>>::new();
    for record in own.type_source_field_records() {
        let Some(owner) = field_owner(bound, record.key())? else {
            continue;
        };

        binding_keys::verify(record.id(), record.key(), bound.foundation.identities)?;
        fields.entry(owner).or_default().insert(record.id());
        bound.fields.insert(record.id(), record.key());
    }
    for record in own.type_source_enum_variant_records() {
        let Some(owner) = record.key().source_owner() else {
            continue;
        };
        if !required(bound, owner, PublicNominalKindV1::Enum)? {
            continue;
        }

        binding_keys::verify(record.id(), record.key(), bound.foundation.identities)?;
        variants.entry(owner).or_default().insert(record.id());
        bound.variants.insert(record.id(), record.key());
    }
    for record in own.type_source_enum_variant_field_records() {
        let variant = record.key().variant();
        if !bound.variants.contains_key(&variant) {
            continue;
        }

        binding_keys::verify(record.id(), record.key(), bound.foundation.identities)?;
        variant_fields
            .entry(variant)
            .or_default()
            .insert(record.id());
        bound.variant_fields.insert(record.id(), record.key());
    }
    for record in own.type_source_object_value_records() {
        let owner = SourceNominalId::from_source_declaration(record.key())
            .map_err(|error| Error::Identity(error.to_string()))?;
        if !required(bound, owner, PublicNominalKindV1::Object)? {
            continue;
        }

        binding_keys::verify(record.id(), record.key(), bound.foundation.identities)?;
        if record.key() != bound.foundation.nominal_key(owner)? {
            return Err(invalid(owner, "object value and nominal keys differ"));
        }
        bound.objects.insert(record.id(), record.key());
    }
    for record in bound.table.records() {
        exact_set(
            fields.get(&record.owner()),
            record
                .source_shape()
                .declared_fields()
                .iter()
                .map(NominalSourceFieldV1::field),
            "nominal fields",
        )?;
        match record.source_shape() {
            NominalSourceShapeV1::Enum(shape) => {
                exact_set(
                    variants.get(&record.owner()),
                    shape.variants().iter().map(EnumSourceVariantV1::variant),
                    "enum variants",
                )?;
                for variant in shape.variants() {
                    exact_set(
                        variant_fields.get(&variant.variant()),
                        variant.fields().iter().map(EnumSourceFieldV1::field),
                        "enum variant fields",
                    )?;
                    let origin =
                        variant_origin(bound.foundation, record.owner(), variant.variant())?;

                    bound.variant_sources.insert(
                        variant.variant(),
                        VariantSource {
                            shape: variant,
                            origin,
                        },
                    );
                }
            }
            NominalSourceShapeV1::Object(shape) => {
                bound.object_value_key(shape.value())?;
            }
            NominalSourceShapeV1::Class(_)
            | NominalSourceShapeV1::Struct(_)
            | NominalSourceShapeV1::Intrinsic(_)
            | NominalSourceShapeV1::Interface => {}
        }
    }
    Ok(())
}

fn required(
    bound: &BoundNominalSourceContractsV1<'_, '_>,
    owner: SourceNominalId,
    kind: PublicNominalKindV1,
) -> Result<bool, Error> {
    Ok(bound
        .table
        .get(owner)
        .is_some_and(|source| source.kind() == kind))
}

fn exact_set<I: Ord + Copy>(
    expected: Option<&BTreeSet<I>>,
    actual: impl ExactSizeIterator<Item = I>,

    name: &'static str,
) -> Result<(), Error> {
    let count = actual.len();
    let actual: BTreeSet<_> = actual.collect();
    if actual.len() != count || !expected.into_iter().flatten().eq(actual.iter()) {
        return Err(Error::Inventory(name));
    }
    Ok(())
}

fn variant_origin(
    foundation: &BoundTypeFoundationSourcesV1<'_>,
    owner: SourceNominalId,
    variant: PersistentEnumVariantId,
) -> Result<ExportDefinitionSourceV1, Error> {
    let subject = DefinitionOriginSubject::EnumVariant(variant);

    let origin = foundation
        .foundation
        .definition_origin(subject)
        .ok_or(Error::Origin(subject))?
        .origin();

    if origin.source()
        != foundation
            .nominal_source(owner)?
            .access()
            .definition_origin()
            .origin()
            .source()
    {
        return Err(Error::Origin(subject));
    }
    let source = ExportDefinitionSourceV1::new(origin.clone());
    foundation.validate_origin(&source)?;
    Ok(source)
}

fn field_owner(
    bound: &BoundNominalSourceContractsV1<'_, '_>,
    key: &FieldIdentityKey,
) -> Result<Option<SourceNominalId>, Error> {
    use scoop_identity::{FieldIdentityView, GeneratedNominalKey};
    let owner = match key.view() {
        FieldIdentityView::SourceDeclared { owner, .. }
        | FieldIdentityView::SourcePropertyBacking { owner, .. }
        | FieldIdentityView::SourcePropertyDelegate { owner, .. } => owner,
        FieldIdentityView::Generated { owner, key } if key.object_backing_property().is_some() => {
            let GeneratedNominalKey::ObjectBackingClass { object } =
                bound.foundation.generated_key(owner)?
            else {
                return Err(Error::Inventory("object backing field owner"));
            };
            SourceNominalId::Concrete(*object)
        }
        FieldIdentityView::Generated { .. } => return Ok(None),
    };

    Ok(bound.table.get(owner).map(|_| owner))
}
