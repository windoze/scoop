use super::*;
use scoop_wire::encoded_length;

impl DecodedNominalRepresentationSupportV1 {
    /// Preflights every inline array, field, signature, and origin before any
    /// resolver lookup or output allocation is attempted.
    pub fn charge_resolution_at(
        &self,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), WireError> {
        node(meter, path, 1, 3)?;
        self.declaration_access
            .charge_resolution_at(meter, &path.clone().field(2), 2)?;
        shape(&self.shape, meter, &path.clone().field(3), 2)?;
        let bytes = encoded_length(self).map_err(|_| overflow(path))?;
        meter.charge_owned_bytes(bytes, path)?;
        meter.charge_work(bytes, path)
    }
}
pub(super) fn shape(
    shape: &DecodedNominalRepresentationShapeV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
    depth: u64,
) -> Result<(), WireError> {
    use DecodedNominalRepresentationShapeV1 as Shape;
    node(meter, path, depth, 2)?;
    let child = depth.checked_add(1).ok_or_else(|| overflow(path))?;
    match shape {
        Shape::Struct {
            fields,
            c_layout_policy,
        } => {
            let at = path.clone().field(1);
            collection(fields.len(), meter, &at)?;
            for (index, field) in fields.iter().enumerate() {
                field.charge_resolution_at(meter, &at.clone().index(index as u64), child)?;
            }
            let at = path.clone().field(2);
            node(meter, &at, child, 1)?;
            if matches!(c_layout_policy, NominalCLayoutPolicyV1::CLayout { .. }) {
                node(
                    meter,
                    &at,
                    child.checked_add(2).ok_or_else(|| overflow(&at))?,
                    2,
                )?;
                meter.charge_nodes(2, &at)?;
            }
        }
        Shape::Enum { variants } => {
            let at = path.clone().field(1);
            collection(variants.len(), meter, &at)?;
            for (index, value) in variants.iter().enumerate() {
                variant(value, meter, &at.clone().index(index as u64), child)?;
            }
        }
        Shape::Class {
            base,
            declared_fields,
        } => {
            let at = path.clone().field(1);
            node(meter, &at, child, 1)?;
            if let DecodedOptionalSignatureType::Present(base) = base {
                let next = child.checked_add(1).ok_or_else(|| overflow(&at))?;
                meter.check_semantic_depth(next, &at)?;
                base.charge_resolution_at(meter, &at.field(1), next)?;
            }
            class_fields(declared_fields, meter, &path.clone().field(2), child)?;
        }
        Shape::Object {
            declared_fields, ..
        } => class_fields(declared_fields, meter, &path.clone().field(2), child)?,
        Shape::Intrinsic { representation } => {
            let at = path.clone().field(1);
            node(meter, &at, child, 1)?;
            if matches!(
                representation.family(),
                crate::IntrinsicTypeKind::Integer(_)
            ) {
                node(
                    meter,
                    &at,
                    child.checked_add(1).ok_or_else(|| overflow(&at))?,
                    2,
                )?;
            }
        }
        Shape::Interface => {}
    }
    Ok(())
}
fn class_fields(
    fields: &[DecodedClassRepresentationFieldV1],
    meter: &mut BudgetMeter,
    path: &WirePath,
    depth: u64,
) -> Result<(), WireError> {
    collection(fields.len(), meter, path)?;
    for (index, field) in fields.iter().enumerate() {
        field.charge_resolution_at(meter, &path.clone().index(index as u64), depth)?;
    }
    Ok(())
}
pub(super) fn variant(
    variant: &DecodedEnumRepresentationVariantV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
    depth: u64,
) -> Result<(), WireError> {
    node(meter, path, depth, 3)?;
    let at = path.clone().field(2);
    collection(variant.fields.len(), meter, &at)?;
    let next = depth.checked_add(1).ok_or_else(|| overflow(path))?;
    for (index, field) in variant.fields.iter().enumerate() {
        field.charge_resolution_at(meter, &at.clone().index(index as u64), next)?;
    }
    Ok(())
}
fn collection(count: usize, meter: &mut BudgetMeter, path: &WirePath) -> Result<(), WireError> {
    let count = count as u64;
    meter.check_table_entries(count, path)?;
    let comparisons = count
        .checked_mul(u64::from(count.max(1).ilog2()) + 1)
        .and_then(|n| n.checked_mul(33))
        .ok_or_else(|| overflow(path))?;
    meter.charge_work(comparisons, path)?;
    meter.charge_collection_slots(count, path)?;
    meter.charge_collection_slots(count, path)?;
    meter.charge_edges(count, path)
}
fn node(meter: &mut BudgetMeter, path: &WirePath, depth: u64, edges: u64) -> Result<(), WireError> {
    meter.check_semantic_depth(depth, path)?;
    meter.charge_nodes(1, path)?;
    meter.charge_edges(edges, path)?;
    meter.charge_work(1 + 32 * edges, path)
}
fn overflow(path: &WirePath) -> WireError {
    WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None)
}
