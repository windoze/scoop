use super::*;
use crate::{
    NestedSourceSupportV1, NominalSupportPropertyPayloadV1, ProtectedNestedNominalPayloadV1,
};

pub(super) fn visit<A: TypeDefinitionSourceSemanticAuthority<E>, E>(
    root: &ProtectedNestedNominalPayloadV1,
    validator: &mut Validator<'_, A>,
    meter: &mut BudgetMeter,
    path: WirePath,
) -> Result<(), TypeDefinitionSourceClosureError<E>> {
    let mut pending = Vec::new();
    meter.check_semantic_depth(1, &path)?;
    meter.try_reserve_collection_slots(&mut pending, 1, &path)?;
    pending.push((root, 1_u64, path));
    while let Some((payload, depth, path)) = pending.pop() {
        meter.charge_nodes(1, &path)?;
        meter.charge_work(1, &path)?;
        let records = payload.source_interface().source_support().records();
        meter.check_table_entries(records.len() as u64, &path)?;
        meter.charge_edges(records.len() as u64, &path)?;
        for (index, record) in records.iter().enumerate() {
            let at = path.clone().field(2).field(9).index(index as u64);
            validator.observe(
                record.declaration_access().definition_origin(),
                TypeDefinitionSourceUseV1::NestedSupport {
                    owner: payload.source_nominal(),
                    declaration: record,
                },
                meter,
                &at.clone().field(2).field(3),
            )?;
            match record {
                NestedSourceSupportV1::Callable(_) | NestedSourceSupportV1::Constructor(_) => {
                    // Parameter origins live in the independent source protocol table.
                }
                NestedSourceSupportV1::Property(property) => match property.payload() {
                    NominalSupportPropertyPayloadV1::Runtime { interface } => setter(
                        property.declaration(),
                        interface,
                        validator,
                        meter,
                        &at.field(3).field(1),
                    )?,
                    NominalSupportPropertyPayloadV1::Const { value } => validator.observe(
                        value.definition_origin(),
                        TypeDefinitionSourceUseV1::NestedConst { property, value },
                        meter,
                        &at.field(3).field(1).field(4),
                    )?,
                },
                NestedSourceSupportV1::NestedNominal(nominal) => {
                    let next = depth.checked_add(1).ok_or_else(|| {
                        scoop_wire::WireError::new(
                            scoop_wire::WireErrorKind::IntegerOutOfRange,
                            at.clone(),
                            None,
                        )
                    })?;
                    meter.check_semantic_depth(next, &at)?;
                    meter.try_reserve_collection_slots(&mut pending, 1, &at)?;
                    pending.push((nominal.payload(), next, at.field(3)));
                }
            }
        }
    }
    Ok(())
}
