use super::*;

pub(super) fn bind(
    bound: &mut BoundNominalSourceContractsV1<'_, '_>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let own = bound.foundation.foundation.as_canonical();
    let path = WirePath::root();
    for count in [
        own.type_source_field_records().len(),
        own.type_source_enum_variant_records().len(),
        own.type_source_enum_variant_field_records().len(),
        own.type_source_object_value_records().len(),
    ] {
        binding_keys::charge_map(count, meter, &path)?;
    }
    let mut fields = BTreeMap::<SourceNominalId, BTreeSet<_>>::new();
    let mut variants = BTreeMap::<SourceNominalId, BTreeSet<_>>::new();
    let mut variant_fields = BTreeMap::<PersistentEnumVariantId, BTreeSet<_>>::new();
    for record in own.type_source_field_records() {
        let Some(owner) = record.key().source_owner() else {
            continue;
        };
        if !required(bound, owner, PublicNominalKindV1::Struct, meter)? {
            continue;
        }
        insert_resources(fields.len(), record.key(), meter)?;
        binding_keys::verify(
            record.id(),
            record.key(),
            bound.foundation.identities,
            meter,
            &path,
        )?;
        fields.entry(owner).or_default().insert(record.id());
        bound.fields.insert(record.id(), record.key());
    }
    for record in own.type_source_enum_variant_records() {
        let Some(owner) = record.key().source_owner() else {
            continue;
        };
        if !required(bound, owner, PublicNominalKindV1::Enum, meter)? {
            continue;
        }
        insert_resources(variants.len(), record.key(), meter)?;
        binding_keys::verify(
            record.id(),
            record.key(),
            bound.foundation.identities,
            meter,
            &path,
        )?;
        variants.entry(owner).or_default().insert(record.id());
        bound.variants.insert(record.id(), record.key());
    }
    for record in own.type_source_enum_variant_field_records() {
        queries(bound.variants.len(), meter)?;
        let variant = record.key().variant();
        if !bound.variants.contains_key(&variant) {
            continue;
        }
        insert_resources(variant_fields.len(), record.key(), meter)?;
        binding_keys::verify(
            record.id(),
            record.key(),
            bound.foundation.identities,
            meter,
            &path,
        )?;
        variant_fields
            .entry(variant)
            .or_default()
            .insert(record.id());
        bound.variant_fields.insert(record.id(), record.key());
    }
    for record in own.type_source_object_value_records() {
        queries(bound.table.records().len(), meter)?;
        NominalRepresentationSupportV1::charge_source_key_resources(record.key(), meter, &path)?;
        let bytes = scoop_wire::encoded_length(record.key())
            .map_err(|error| Error::Identity(error.to_string()))?;
        meter.charge_sha256(bytes, &path)?;
        let owner = SourceNominalId::from_source_declaration(record.key())
            .map_err(|error| Error::Identity(error.to_string()))?;
        if !required(bound, owner, PublicNominalKindV1::Object, meter)? {
            continue;
        }
        insert_resources(bound.objects.len(), record.key(), meter)?;
        NominalRepresentationSupportV1::charge_source_key_resources(record.key(), meter, &path)?;
        binding_keys::verify(
            record.id(),
            record.key(),
            bound.foundation.identities,
            meter,
            &path,
        )?;
        if record.key() != bound.foundation.nominal_key(owner)? {
            return Err(invalid(owner, "object value and nominal keys differ"));
        }
        bound.objects.insert(record.id(), record.key());
    }
    for record in bound.table.records() {
        match record.source_shape() {
            NominalSourceShapeV1::Struct(shape) => {
                exact_set(
                    fields.get(&record.owner()),
                    shape.fields().iter().map(StructSourceFieldV1::field),
                    meter,
                    "struct fields",
                )?;
            }
            NominalSourceShapeV1::Enum(shape) => {
                exact_set(
                    variants.get(&record.owner()),
                    shape.variants().iter().map(EnumSourceVariantV1::variant),
                    meter,
                    "enum variants",
                )?;
                for variant in shape.variants() {
                    exact_set(
                        variant_fields.get(&variant.variant()),
                        variant.fields().iter().map(EnumSourceFieldV1::field),
                        meter,
                        "enum variant fields",
                    )?;
                    let origin =
                        variant_origin(bound.foundation, record.owner(), variant.variant(), meter)?;
                    insert_resources(bound.variant_sources.len(), &origin, meter)?;
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
            NominalSourceShapeV1::Class | NominalSourceShapeV1::Interface => {}
        }
    }
    Ok(())
}

fn required(
    bound: &BoundNominalSourceContractsV1<'_, '_>,
    owner: SourceNominalId,
    kind: PublicNominalKindV1,
    meter: &mut BudgetMeter,
) -> Result<bool, Error> {
    queries(bound.table.records().len(), meter)?;
    Ok(bound
        .table
        .get(owner)
        .is_some_and(|source| source.kind() == kind))
}

fn insert_resources<K: scoop_wire::WireEncode>(
    length: usize,
    key: &K,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let path = WirePath::root();
    meter.check_table_entries(length as u64 + 1, &path)?;
    meter.charge_nodes(1, &path)?;
    // Borrowed key index, owner index, and its per-owner set entry.
    meter.charge_collection_slots(3, &path)?;
    queries(length, meter)?;
    let bytes =
        scoop_wire::encoded_length(key).map_err(|error| Error::Identity(error.to_string()))?;
    meter.check_semantic_leaf(bytes, &path)?;
    meter.charge_work(bytes, &path)?;
    Ok(())
}

fn exact_set<I: Ord + Copy>(
    expected: Option<&BTreeSet<I>>,
    actual: impl ExactSizeIterator<Item = I>,
    meter: &mut BudgetMeter,
    name: &'static str,
) -> Result<(), Error> {
    let path = WirePath::root();
    binding_keys::charge_map(actual.len(), meter, &path)?;
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
    meter: &mut BudgetMeter,
) -> Result<ExportDefinitionSourceV1, Error> {
    let path = WirePath::root();
    let subject = DefinitionOriginSubject::EnumVariant(variant);
    queries(
        foundation
            .foundation
            .as_canonical()
            .counts()
            .definition_origins,
        meter,
    )?;
    let origin = foundation
        .foundation
        .definition_origin(subject)
        .ok_or(Error::Origin(subject))?
        .origin();
    let bytes =
        scoop_wire::encoded_length(origin).map_err(|error| Error::Identity(error.to_string()))?;
    meter.check_semantic_leaf(bytes, &path)?;
    meter.charge_owned_bytes(bytes, &path)?;
    meter.charge_work(bytes, &path)?;
    queries(foundation.source().entries().sources.records().len(), meter)?;
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
    foundation.validate_origin(&source, meter, &path)?;
    Ok(source)
}
