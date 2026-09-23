use super::*;
use MirTypeBridgeSourceProjectionError as Error;

pub(super) fn project(
    input: MirTypeBridgeExportInputV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<Vec<mir::MirTypeBridgeDependencyV1>, Error> {
    let module = input.mir.module();
    let path = WirePath::root();
    let mut uses = Vec::new();
    meter.charge_work(module.meta.source_exact_types.len() as u64, &path)?;
    for source in module.meta.source_exact_types.iter() {
        if let mir::SourceExactTypeOwner::Cone(provider) = source.owner()
            && provider != module.cone
        {
            push(
                &mut uses,
                mir::MirTypeBridgeDependencyV1::new(
                    provider,
                    mir::MirTypeBridgeTargetV1::Type(source.identity_record().id()),
                ),
                meter,
            )?;
        }
    }
    for root in input.mir.materialization().external_callable_roots() {
        meter.charge_work(input.ordinary.selected().len() as u64 + 1, &path)?;
        if root.role() == mir::CallableRole::InitializationCycle
            || input.ordinary.selected().iter().any(|selected| {
                selected.provider() == root.provider()
                    && selected.declaration() == root.declaration()
                    && selected.implementation() == root.implementation()
                    && selected.signature() == root.signature()
            })
        {
            continue;
        }
        push(
            &mut uses,
            mir::MirTypeBridgeDependencyV1::new(
                root.provider(),
                mir::MirTypeBridgeTargetV1::Callable(root.implementation()),
            ),
            meter,
        )?;
    }
    inventory::sort_cost(uses.len(), meter)?;
    uses.sort_unstable();
    uses.dedup();
    Ok(uses)
}

fn push(
    uses: &mut Vec<mir::MirTypeBridgeDependencyV1>,
    relation: mir::MirTypeBridgeDependencyV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let path = WirePath::root();
    meter.check_table_entries(uses.len() as u64 + 1, &path)?;
    meter.charge_owned_bytes(std::mem::size_of_val(&relation) as u64, &path)?;
    meter.try_reserve_collection_slots(uses, 1, &path)?;
    uses.push(relation);
    Ok(())
}
