use super::*;
use MeteredNominalRepresentationResolutionError as Error;

pub(super) fn shape<R: NominalRepresentationResolver<E>, E>(
    shape: DecodedNominalRepresentationShapeV1,
    resolver: &mut R,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<NominalRepresentationShapeV1, Error<E>> {
    use DecodedNominalRepresentationShapeV1 as Decoded;
    Ok(match shape {
        Decoded::Struct {
            fields,
            c_layout_policy,
        } => {
            let mut values = reserved(fields.len(), path)?;
            for (index, field) in fields.into_iter().enumerate() {
                values.push(
                    field
                        .resolve_precharged(
                            resolver,
                            meter,
                            &path.clone().field(1).index(index as u64),
                        )
                        .map_err(field_error)?,
                );
            }
            NominalRepresentationShapeV1::Struct {
                fields: values,
                c_layout_policy,
            }
        }
        Decoded::Enum { variants } => {
            let mut values = reserved(variants.len(), path)?;
            for (index, value) in variants.into_iter().enumerate() {
                values.push(variant(
                    value,
                    resolver,
                    meter,
                    &path.clone().field(1).index(index as u64),
                )?);
            }
            NominalRepresentationShapeV1::Enum { variants: values }
        }
        Decoded::Class {
            base,
            declared_fields,
        } => NominalRepresentationShapeV1::Class {
            base: base
                .resolve(resolver)
                .map_err(|e| Error::Value(NominalRepresentationResolutionError::Reference(e)))?,
            declared_fields: class_fields(
                declared_fields,
                resolver,
                meter,
                &path.clone().field(2),
            )?,
        },
        Decoded::Object {
            backing_class,
            declared_fields,
        } => NominalRepresentationShapeV1::Object {
            backing_class: resolver
                .resolve(backing_class)
                .map_err(|e| Error::Value(NominalRepresentationResolutionError::Reference(e)))?,
            declared_fields: class_fields(
                declared_fields,
                resolver,
                meter,
                &path.clone().field(2),
            )?,
        },
        Decoded::Interface => NominalRepresentationShapeV1::Interface,
        Decoded::Intrinsic { representation } => {
            NominalRepresentationShapeV1::Intrinsic { representation }
        }
    })
}
fn class_fields<R: NominalRepresentationResolver<E>, E>(
    fields: Vec<DecodedClassRepresentationFieldV1>,
    resolver: &mut R,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<Vec<ClassRepresentationFieldV1>, Error<E>> {
    let mut values = reserved(fields.len(), path)?;
    for (index, field) in fields.into_iter().enumerate() {
        values.push(
            field
                .resolve_precharged(resolver, meter, &path.clone().index(index as u64))
                .map_err(field_error)?,
        );
    }
    Ok(values)
}
fn variant<R: NominalRepresentationResolver<E>, E>(
    value: DecodedEnumRepresentationVariantV1,
    resolver: &mut R,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<EnumRepresentationVariantV1, Error<E>> {
    let key = resolver
        .resolve_key(value.variant)
        .map_err(|e| Error::Value(NominalRepresentationResolutionError::Reference(e)))?;
    let at = path.clone().field(1);
    meter.charge_nodes(2, &at)?;
    meter.charge_edges(2, &at)?;
    if let Some(name) = key.source_name() {
        meter.check_semantic_leaf(name.as_str().len() as u64, &at)?;
        meter.charge_work(name.as_str().len() as u64, &at)?;
    }
    let bytes = scoop_wire::encoded_length(key.as_ref())
        .map_err(|_| WireError::new(WireErrorKind::IntegerOutOfRange, at.clone(), None))?;
    meter.charge_work(bytes, &at)?;
    let mut fields = reserved(value.fields.len(), path)?;
    for (index, field) in value.fields.into_iter().enumerate() {
        fields.push(
            field
                .resolve_precharged(resolver, meter, &path.clone().field(2).index(index as u64))
                .map_err(|error| match error {
                    MeteredRepresentationFieldResolutionError::Resource(error) => {
                        Error::Resource(error)
                    }
                    MeteredRepresentationFieldResolutionError::Value(error) => {
                        Error::Value(NominalRepresentationResolutionError::EnumField(error))
                    }
                })?,
        );
    }
    let record = EnumRepresentationVariantV1::try_new(&key, fields, value.gc)
        .map_err(|e| Error::Value(NominalRepresentationResolutionError::Record(e)))?;
    value
        .variant
        .verify(record.variant())
        .map_err(|e| Error::Value(NominalRepresentationResolutionError::VariantIdentity(e)))?;
    Ok(record)
}
fn field_error<E>(
    error: MeteredRepresentationFieldResolutionError<E, PersistentFieldId>,
) -> Error<E> {
    match error {
        MeteredRepresentationFieldResolutionError::Resource(error) => Error::Resource(error),
        MeteredRepresentationFieldResolutionError::Value(error) => {
            Error::Value(NominalRepresentationResolutionError::Field(error))
        }
    }
}

// Output collection capacity was charged by the complete inline preflight.
fn reserved<T>(count: usize, path: &WirePath) -> Result<Vec<T>, WireError> {
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|_| {
        WireError::new(
            WireErrorKind::ResourceAllocation {
                requested_logical_bytes: (count as u64)
                    .saturating_mul(scoop_wire::budget::COLLECTION_ELEMENT_BYTES),
                requested_slots: count as u64,
            },
            path.clone(),
            None,
        )
    })?;
    Ok(values)
}
