use super::*;
use MirTypeBridgeSourceProjectionError as Error;

pub(super) fn project(
    input: MirTypeBridgeExportInputV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<Vec<mir::MirTypeBridgeDependencyV1>, Error> {
    let module = input.mir.module();
    let path = WirePath::root();
    let mut uses = Vec::new();
    let local = &input.hir.output().local;
    for ty in local
        .materialized_type_closure(meter)
        .map_err(Error::MaterializedTypes)?
    {
        meter.charge_work(1, &path)?;
        let exact = &local.exact_type_identities[ty];
        let scoop_identity::ExactTypeKey::Nominal(source) = exact.key() else {
            continue;
        };
        let declaration = input
            .identities
            .canonical_key::<_, scoop_identity::SourceDeclarationKey>(*source)
            .map_err(Error::Identity)?;
        let provider = declaration.origin();
        if provider != module.cone {
            push(
                &mut uses,
                mir::MirTypeBridgeDependencyV1::new(
                    provider,
                    mir::MirTypeBridgeTargetV1::Type(exact.id()),
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
