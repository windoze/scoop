use super::*;

mod entries;
mod schemas;

pub(super) fn lower(
    input: LayoutAbiExportInputV1<'_>,
    layouts: &lookup::Layouts<'_>,
    callables: &lir::CanonicalExactCallableAbiExportsV1,
    dependencies: &[&lir::CanonicalExactCallableAbiExportsV1],
) -> Result<lir::CanonicalExactDispatchExportsV1, Error> {
    let output = input.lir;
    let descriptors = &output.module().meta.type_descriptors;

    let count = descriptors
        .iter()
        .try_fold(0_usize, |count, (_, descriptor)| {
            count.checked_add(1)?.checked_add(descriptor.itables.len())
        })
        .ok_or(Error::CountOverflow)?;
    let mut records = reserve(count)?;
    for (_, descriptor) in output.module().meta.type_descriptors.iter() {
        let owner = descriptor.identity.exact_type();

        if input.bridge.types().get(owner).is_none() {
            continue;
        }
        let schema = schemas::for_owner(input.bridge, owner)?;
        let slots = schema.vtable();
        let entries = entries::project(slots, layouts, callables, dependencies)?;
        records.push(replay(input, (&descriptor.vtable).into(), &entries)?);
        for table in &descriptor.itables {
            let key = table.identity_record().key();
            let scoop_identity::OptionalExactInterface::Present(interface) = key.interface() else {
                return Err(Error::MissingInterface(table.identity_record().id()));
            };
            let slots = schema
                .interface(interface)
                .ok_or(Error::MissingInterface(table.identity_record().id()))?;
            let entries = entries::project(slots, layouts, callables, dependencies)?;
            records.push(replay(input, table.into(), &entries)?);
        }
    }
    Ok(lir::CanonicalExactDispatchExportsV1::try_new(
        output.module().meta.target_profile,
        output.foundation(),
        records,
    )?)
}

fn replay(
    input: LayoutAbiExportInputV1<'_>,
    physical: lir::ExactDispatchPhysicalTableV1<'_>,
    entries: &[lir::ExactDispatchEntryInputV1<'_>],
) -> Result<lir::ExactDispatchExportV1, Error> {
    let module = input.lir.module();
    let mut resolver = |callable| {
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
    )?)
}
