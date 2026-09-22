use super::*;

mod entries;
mod schemas;

pub(super) fn lower(
    input: LayoutAbiExportInputV1<'_>,
    layouts: &lookup::Layouts<'_>,
    callables: &lir::CanonicalExactCallableAbiExportsV1,
    dependencies: &[&lir::CanonicalExactCallableAbiExportsV1],
    meter: &mut BudgetMeter,
) -> Result<lir::CanonicalExactDispatchExportsV1, Error> {
    let output = input.lir;
    let descriptors = &output.module().meta.type_descriptors;
    meter.charge_work(descriptors.len() as u64, &WirePath::root())?;
    let count = descriptors
        .iter()
        .try_fold(0_usize, |count, (_, descriptor)| {
            count.checked_add(1)?.checked_add(descriptor.itables.len())
        })
        .ok_or(Error::CountOverflow)?;
    let mut records = reserve(count, meter)?;
    for (_, descriptor) in output.module().meta.type_descriptors.iter() {
        let owner = descriptor.identity.exact_type();
        let schema = schemas::for_owner(input.bridge, owner, meter, 1)?;
        let slots = schema.vtable();
        let entries = entries::project(slots, layouts, callables, dependencies, meter)?;
        records.push(replay(input, (&descriptor.vtable).into(), &entries, meter)?);
        for table in &descriptor.itables {
            let key = table.identity_record().key();
            let scoop_identity::OptionalExactInterface::Present(interface) = key.interface() else {
                return Err(Error::MissingInterface(table.identity_record().id()));
            };
            let slots = schema
                .interface(interface)
                .ok_or(Error::MissingInterface(table.identity_record().id()))?;
            let entries = entries::project(slots, layouts, callables, dependencies, meter)?;
            records.push(replay(input, table.into(), &entries, meter)?);
        }
    }
    Ok(lir::CanonicalExactDispatchExportsV1::try_new(
        output.module().meta.target_profile,
        output.foundation(),
        records,
        meter,
    )?)
}

fn replay(
    input: LayoutAbiExportInputV1<'_>,
    physical: lir::ExactDispatchPhysicalTableV1<'_>,
    entries: &[lir::ExactDispatchEntryInputV1<'_>],
    meter: &mut BudgetMeter,
) -> Result<lir::ExactDispatchExportV1, Error> {
    let module = input.lir.module();
    let mut resolver = |callable, meter: &mut BudgetMeter| {
        meter.charge_work(1, &WirePath::root())?;
        Ok(match callable {
            lir::CallableRef::Local(id) => {
                module
                    .functions
                    .get(id.into_u32() as usize)
                    .map(|function| {
                        lir::StrongTypeDispatchCallableRefV2::Local(function.callable_body.id())
                    })
            }
            lir::CallableRef::External(id) => ((id.into_raw().into_u32() as usize)
                < module.meta.external_callables.len())
            .then(|| {
                let callable = &module.meta.external_callables[id];
                lir::StrongTypeDispatchCallableRefV2::DependencyExternal {
                    provider: callable.provider(),
                    body: callable.body(),
                }
            }),
            lir::CallableRef::Runtime(function) => {
                Some(lir::StrongTypeDispatchCallableRefV2::Runtime(function))
            }
        })
    };
    Ok(lir::ExactDispatchExportV1::replay(
        module.meta.target_profile,
        physical,
        entries,
        input.lir.foundation(),
        &mut resolver,
        meter,
    )?)
}
