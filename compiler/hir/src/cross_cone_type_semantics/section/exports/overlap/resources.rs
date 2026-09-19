use super::*;

pub(super) fn sequence(
    count: usize,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), WireError> {
    meter.check_table_entries(count as u64, path)?;
    meter.charge_nodes(count as u64, path)?;
    meter.charge_edges(count as u64, path)?;
    meter.charge_work(
        (count as u64)
            .checked_mul(64)
            .ok_or_else(|| overflow(path))?,
        path,
    )
}
pub(super) fn lookup(
    count: usize,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), WireError> {
    meter.check_table_entries(count as u64, path)?;
    meter.charge_work(
        64 * u64::from(u64::BITS - (count as u64).leading_zeros()),
        path,
    )
}
pub(super) fn text(value: &str, meter: &mut BudgetMeter, path: &WirePath) -> Result<(), WireError> {
    meter.check_semantic_leaf(value.len() as u64, path)?;
    meter.charge_work(value.len() as u64, path)
}
pub(super) fn origin(
    value: &ExportDefinitionSourceV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), WireError> {
    meter.check_semantic_depth(3, path)?;
    meter.charge_nodes(3, path)?;
    meter.charge_work(96, path)?;
    text(value.origin().source().logical_path().as_str(), meter, path)
}
pub(super) fn signature(
    left: &SignatureTypeKey,
    right: &SignatureTypeKey,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<bool, WireError> {
    NominalRepresentationSupportV1::signature_types_match_metered(left, right, 1, meter, path)
}
pub(super) fn signatures(
    left: &[SignatureTypeKey],
    right: &[SignatureTypeKey],
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<bool, WireError> {
    sequence(left.len(), meter, path)?;
    sequence(right.len(), meter, path)?;
    if left.len() != right.len() {
        return Ok(false);
    }
    for (index, (left, right)) in left.iter().zip(right).enumerate() {
        if !signature(left, right, meter, &path.clone().index(index as u64))? {
            return Ok(false);
        }
    }
    Ok(true)
}
pub(super) fn optional_signature(
    left: Option<&SignatureTypeKey>,
    right: Option<&SignatureTypeKey>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<bool, WireError> {
    match (left, right) {
        (None, None) => Ok(true),
        (Some(left), Some(right)) => signature(left, right, meter, path),
        _ => Ok(false),
    }
}
pub(super) fn binders(
    left: &CanonicalBinderListV1,
    right: &CanonicalBinderListV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<bool, WireError> {
    sequence(left.binders().len(), meter, path)?;
    sequence(right.binders().len(), meter, path)?;
    if left.len_u32() != right.len_u32() {
        return Ok(false);
    }
    for (index, (left, right)) in left.binders().iter().zip(right.binders()).enumerate() {
        let at = path.clone().index(index as u64);
        text(left.name().as_str(), meter, &at)?;
        text(right.name().as_str(), meter, &at)?;
        if left.name() != right.name() {
            return Ok(false);
        }
        match (left.bounds(), right.bounds()) {
            (TypeParameterBoundsV1::Nominal(left), TypeParameterBoundsV1::Nominal(right)) => {
                if !optional_signature(left.class(), right.class(), meter, &at)?
                    || !signatures(
                        left.interfaces().values(),
                        right.interfaces().values(),
                        meter,
                        &at,
                    )?
                {
                    return Ok(false);
                }
            }
            (TypeParameterBoundsV1::Unconstrained, TypeParameterBoundsV1::Unconstrained)
            | (TypeParameterBoundsV1::Value, TypeParameterBoundsV1::Value)
            | (TypeParameterBoundsV1::Ref, TypeParameterBoundsV1::Ref) => {}
            _ => return Ok(false),
        }
    }
    Ok(true)
}
pub(super) fn constant(
    value: &CanonicalConstValueV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), WireError> {
    meter.charge_nodes(1, path)?;
    meter.charge_work(8, path)?;
    if let CanonicalConstValueV1::String(value) = value {
        text(value, meter, path)?;
    }
    Ok(())
}
pub(super) fn overflow(path: &WirePath) -> WireError {
    WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None)
}
