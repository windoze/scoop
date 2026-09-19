use super::*;
use scoop_wire::WireError;

pub(super) fn access(
    value: &DeclarationAccessSourceV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), WireError> {
    sequence(value.lexical_owners().len(), meter, path)?;
    text(
        value
            .definition_origin()
            .origin()
            .source()
            .logical_path()
            .as_str(),
        meter,
        path,
    )
}
pub(super) fn domain(
    value: &PersistentAccessDomainV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), WireError> {
    sequence(value.constraints().len(), meter, path)?;
    for constraint in value.constraints() {
        if let PersistentAccessConstraintV1::File(source) = constraint {
            text(source.logical_path().as_str(), meter, path)?;
        }
    }
    Ok(())
}
fn sequence(count: usize, meter: &mut BudgetMeter, path: &WirePath) -> Result<(), WireError> {
    meter.check_table_entries(count as u64, path)?;
    meter.check_semantic_depth(4, path)?;
    meter.charge_nodes(count as u64 + 1, path)?;
    meter.charge_work((count as u64).saturating_mul(64).saturating_add(128), path)
}
fn text(value: &str, meter: &mut BudgetMeter, path: &WirePath) -> Result<(), WireError> {
    meter.check_semantic_leaf(value.len() as u64, path)?;
    meter.charge_work(value.len() as u64, path)
}
