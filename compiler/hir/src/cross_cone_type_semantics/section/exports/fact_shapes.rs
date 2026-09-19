use super::*;
use scoop_identity::{PersistentExactTypeId, SignatureTypeKey};

pub(super) fn validate<F: TypeSectionFoundationSemanticAuthority<E>, E>(
    representations: CheckedNominalRepresentationSupportV1<'_>,
    foundation: &F,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), TypeSectionExportValidationError<E>> {
    use TypeSectionExportValidationError as Error;
    for record in representations.table().records() {
        let exact = inventory::nominal_exact(record.owner(), meter, path)?;
        let shape = foundation.fact_shape(exact).map_err(Error::Source)?;
        meter.charge_work(1, path)?;
        match (record.shape(), shape) {
            (
                NominalRepresentationShapeV1::Interface
                | NominalRepresentationShapeV1::Object { .. },
                ExactTypeFactShapeV1::Reference,
            ) => {}
            (NominalRepresentationShapeV1::Class { base, .. }, ExactTypeFactShapeV1::Reference) => {
                class_base(exact, base, foundation, meter, path)?;
            }
            (
                NominalRepresentationShapeV1::Struct {
                    fields,
                    c_layout_policy,
                },
                shape,
            ) => {
                let exact_fields = match (c_layout_policy, shape) {
                    (
                        NominalCLayoutPolicyV1::Ordinary,
                        ExactTypeFactShapeV1::OrdinaryStruct { fields },
                    )
                    | (
                        NominalCLayoutPolicyV1::CLayout { .. },
                        ExactTypeFactShapeV1::CLayoutStruct { fields },
                    ) => fields.as_slice(),
                    (NominalCLayoutPolicyV1::Ordinary, ExactTypeFactShapeV1::Unit)
                        if fields.is_empty() =>
                    {
                        &[]
                    }
                    _ => return Err(Error::FactShape(exact)),
                };
                fields_match(
                    fields.iter().map(StructRepresentationFieldV1::value_type),
                    exact_fields,
                    foundation,
                    meter,
                    path,
                )?;
            }
            (
                NominalRepresentationShapeV1::Enum { variants },
                ExactTypeFactShapeV1::Enum { variants: expected },
            ) => {
                meter.check_table_entries(variants.len() as u64, path)?;
                meter.charge_work(variants.len() as u64 + expected.len() as u64, path)?;
                if variants.len() != expected.len() {
                    return Err(Error::FactShape(exact));
                }
                for (actual, expected) in variants.iter().zip(expected) {
                    if actual.variant() != expected.variant || actual.gc() != expected.gc {
                        return Err(Error::FactShape(exact));
                    }
                    fields_match(
                        actual
                            .fields()
                            .iter()
                            .map(EnumRepresentationFieldV1::value_type),
                        &expected.fields,
                        foundation,
                        meter,
                        path,
                    )?;
                }
            }
            (NominalRepresentationShapeV1::Intrinsic { representation }, shape) => {
                let valid = match representation.family() {
                    IntrinsicTypeKind::Integer(_) | IntrinsicTypeKind::Boolean => {
                        matches!(shape, ExactTypeFactShapeV1::Scalar)
                    }
                    IntrinsicTypeKind::String => matches!(shape, ExactTypeFactShapeV1::Reference),
                    IntrinsicTypeKind::Array
                    | IntrinsicTypeKind::MutableArray
                    | IntrinsicTypeKind::Ptr
                    | IntrinsicTypeKind::FunPtr => false,
                };
                if !valid {
                    return Err(Error::FactShape(exact));
                }
            }
            _ => return Err(Error::FactShape(exact)),
        }
    }
    Ok(())
}

fn class_base<F: TypeSectionFoundationSemanticAuthority<E>, E>(
    owner: PersistentExactTypeId,
    base: &scoop_identity::OptionalSignatureType,
    foundation: &F,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), TypeSectionExportValidationError<E>> {
    use TypeSectionExportValidationError as Error;
    let edges = foundation
        .local_inheritance_edges()
        .map_err(Error::Source)?;
    meter.charge_work((edges.len() as u64 + 1).ilog2() as u64 + 1, path)?;
    let edge = edges
        .binary_search_by_key(&owner, NominalInheritanceEdgesV1::owner)
        .ok()
        .map(|index| &edges[index])
        .ok_or(Error::FactShape(owner))?;
    match (base, edge.direct_base()) {
        (scoop_identity::OptionalSignatureType::Absent, DirectClassBaseV1::NoClassBase) => Ok(()),
        (
            scoop_identity::OptionalSignatureType::Present(source),
            DirectClassBaseV1::ClassBase { exact },
        ) => fields_match(
            std::iter::once(source.as_ref()),
            std::slice::from_ref(&exact),
            foundation,
            meter,
            path,
        ),
        _ => Err(Error::FactShape(owner)),
    }
}

fn fields_match<'a, F: TypeSectionFoundationSemanticAuthority<E>, E>(
    source: impl ExactSizeIterator<Item = &'a SignatureTypeKey>,
    exact: &[PersistentExactTypeId],
    foundation: &F,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), TypeSectionExportValidationError<E>> {
    meter.check_table_entries(source.len() as u64, path)?;
    meter.check_table_entries(exact.len() as u64, path)?;
    meter.charge_work(source.len() as u64 + exact.len() as u64, path)?;
    if source.len() != exact.len() {
        return Err(TypeSectionExportValidationError::FactType(Box::new(
            InheritanceSlotContractSemanticError::Signature,
        )));
    }
    for (source, exact) in source.zip(exact) {
        NominalRepresentationSupportV1::validate_exact_field_types(
            std::slice::from_ref(source),
            std::slice::from_ref(exact),
            foundation,
            meter,
        )
        .map_err(|error| TypeSectionExportValidationError::FactType(Box::new(error)))?;
    }
    Ok(())
}
