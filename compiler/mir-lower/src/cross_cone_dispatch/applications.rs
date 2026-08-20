//! Project concrete HIR slot identities onto the actual MIR dispatch tables.

use super::*;
use scoop_identity::{CallableTemplateOrigin, GeneratedCallableKey};

pub(super) fn append(
    local: &hir::LocalConcreteHir,
    context: &Context<'_>,
    types: &mir::CanonicalParamFreeMirTypeExportsV1,
    records: &mut Vec<mir::ParamFreeMirDispatchSchemaV1>,
) -> Result<(), Error> {
    let slots = virtual_slots(local, context)?;
    for record in types.records() {
        let owner = record.exact();
        let physical_owner = match record.origin() {
            mir::MirTypeOriginV1::NominalApplication(_) => owner,
            mir::MirTypeOriginV1::GeneratedNominal {
                role: scoop_identity::GeneratedNominalKey::BoxedValue { payload },
                ..
            } if matches!(
                context
                    .authority
                    .identities
                    .canonical_key::<_, scoop_identity::ExactTypeKey>(*payload)?
                    .as_ref(),
                scoop_identity::ExactTypeKey::Tuple(_)
                    | scoop_identity::ExactTypeKey::RawPointer(_)
            ) && !record.base_and_interfaces().interfaces.is_empty() =>
            {
                *payload
            }
            _ => continue,
        };
        let physical = context
            .physical
            .get(&physical_owner)
            .ok_or(Error::MissingPhysicalType(physical_owner))?;
        let mut vtable = mir::MirDispatchSlotsV1::NoClassVtable;
        let mut itables = Vec::new();
        if let mir::Type::Interface(_) = physical {
            vtable = mir::MirDispatchSlotsV1::InterfaceSlots(slots::interface(local, owner)?);
        } else {
            let class = match physical {
                mir::Type::Class(id) => Some(&context.input.module().classes[*id]),
                _ => context
                    .input
                    .module()
                    .meta
                    .boxed_types
                    .iter()
                    .find(|boxed| boxed.payload() == *physical)
                    .map(|boxed| &context.input.module().classes[boxed.class()]),
            };
            let Some(class) = class else {
                // An unboxed value has no runtime dispatch table. Its semantic
                // conformance remains in the type record and HIR declaration.
                continue;
            };
            if matches!(physical, mir::Type::Class(_)) {
                let mut entries = Vec::new();
                for (position, target) in class.vtable.iter().enumerate() {
                    let target = physical_target(context, target)?;
                    let slot = *slots.get(&target).ok_or(Error::Application(owner))?;
                    let contract = virtual_contract(context, owner, slot, position)?;
                    entries.push(entry(context, owner, contract, target)?);
                }
                vtable = mir::MirDispatchSlotsV1::ClassVtable(entries);
            }
            for table in &class.itables {
                let interface = context
                    .input
                    .module()
                    .meta
                    .source_exact_types
                    .get(&mir::Type::Interface(table.interface))
                    .ok_or(Error::Application(owner))?
                    .identity_record()
                    .id();
                let entries = interface_entries(local, context, owner, interface, &table.slots)?;
                itables.push(mir::MirInterfaceDispatchTableV1::new(interface, entries));
            }
        }
        itables.sort_unstable_by_key(mir::MirInterfaceDispatchTableV1::interface);
        records.push(mir::ParamFreeMirDispatchSchemaV1::try_new(
            context.authority,
            owner,
            vtable,
            itables,
        )?);
    }
    Ok(())
}

fn virtual_slots(
    local: &hir::LocalConcreteHir,
    context: &Context<'_>,
) -> Result<BTreeMap<CallableDefinitionOwner, PersistentDispatchSlotId>, Error> {
    let mut slots = BTreeMap::new();
    for (_, function) in local.functions.iter() {
        let Some(method) = function.receiver.method() else {
            continue;
        };
        let family = match method.dispatch {
            hir::concrete::MethodDispatch::Virtual(family)
            | hir::concrete::MethodDispatch::FinalOverride(family) => family,
            _ => continue,
        };
        if let Some(root) = context
            .input
            .module()
            .meta
            .source_callable_materializations
            .get_by_materialization(function.materialization)
        {
            slots.insert(
                context.target(root.function())?,
                local.dispatch_slot_identities.virtual_slot(family).id(),
            );
        }
    }
    for (_, class) in local.classes.iter() {
        for method in &class.methods {
            if let hir::concrete::ClassMethod::Imported { family, callable } = *method {
                let target = match local.imported_dependency_callables[callable]
                    .reference()
                    .declaration()
                {
                    CallableTemplateOrigin::Function(id) => {
                        StrongCallableDefinitionOwner::Function(id)
                    }
                    CallableTemplateOrigin::Accessor(id) => {
                        StrongCallableDefinitionOwner::PropertyAccessor(id)
                    }
                    _ => {
                        return Err(Error::Application(
                            local.exact_type_identities[class.canonical_type].id(),
                        ));
                    }
                };
                slots.insert(
                    target.into(),
                    local.dispatch_slot_identities.virtual_slot(family).id(),
                );
            }
        }
    }
    for generated in context.input.module().meta.generated_callables.iter() {
        if let GeneratedCallableKey::DispatchAdjust { slot, .. } = generated.identity_record().key()
        {
            slots.insert(context.target(generated.function())?, *slot);
        }
    }
    Ok(slots)
}

fn interface_entries(
    local: &hir::LocalConcreteHir,
    context: &Context<'_>,
    owner: PersistentExactTypeId,
    interface: PersistentExactTypeId,
    physical: &[mir::TableSlot],
) -> Result<Vec<mir::MirDispatchEntryV1>, Error> {
    let contracts = slots::interface(local, interface)?;
    if physical.len() != contracts.len() {
        return Err(Error::Application(interface));
    }
    let mut entries = reserve(contracts.len())?;
    for (contract, target) in contracts.into_iter().zip(physical) {
        entries.push(entry(
            context,
            owner,
            contract,
            physical_target(context, target)?,
        )?);
    }
    Ok(entries)
}

fn physical_target(
    context: &Context<'_>,
    slot: &mir::TableSlot,
) -> Result<CallableDefinitionOwner, Error> {
    match *slot {
        mir::TableSlot::Function(function) => context.target(function),
        mir::TableSlot::External(callable) => Ok(context.input.module().meta.external_callables
            [callable]
            .reference()
            .implementation()
            .into()),
        mir::TableSlot::Runtime(runtime) => Err(Error::RuntimeSlot(runtime)),
    }
}

fn virtual_contract(
    context: &Context<'_>,
    owner: PersistentExactTypeId,
    slot: PersistentDispatchSlotId,
    position: usize,
) -> Result<mir::MirDispatchSlotV1, Error> {
    let key = context
        .authority
        .identities
        .canonical_key::<_, DispatchSlotKey>(slot)?;
    let declaration = mir::dispatch_declaration_target(
        context.authority.identities,
        context.authority.types,
        key.owner(),
        owner,
    )?;
    Ok(mir::MirDispatchSlotV1::new(
        slot,
        mir::MirDispatchPositionV1::new(
            u32::try_from(position).map_err(|_| Error::Application(owner))?,
        ),
        context.callable(declaration)?.lowered_signature().clone(),
    ))
}

pub(super) fn entry(
    context: &Context<'_>,
    owner: PersistentExactTypeId,
    contract: mir::MirDispatchSlotV1,
    target: CallableDefinitionOwner,
) -> Result<mir::MirDispatchEntryV1, Error> {
    let binding = context.callable(target)?;
    let receiver = if contract.signature() == binding.lowered_signature() {
        mir::MirDispatchReceiverAdaptationV1::Identity
    } else {
        mir::MirDispatchReceiverAdaptationV1::ReferenceDispatch
    };
    let implementation = match binding.lowering_role() {
        mir::MirCallableLoweringRoleV1::PureVirtualTrap { .. } => {
            let declaration = match binding.origin().as_ref() {
                mir::MirCallableOriginV1::Function(id) => DispatchDeclarationOwner::Function(*id),
                mir::MirCallableOriginV1::Accessor(id) => DispatchDeclarationOwner::Accessor(*id),
                mir::MirCallableOriginV1::Application(application) => {
                    match context
                        .authority
                        .identities
                        .canonical_key::<_, scoop_identity::CallableApplicationKey>(*application)?
                        .origin()
                    {
                        CallableTemplateOrigin::Function(id) => {
                            DispatchDeclarationOwner::Function(id)
                        }
                        CallableTemplateOrigin::Accessor(id) => {
                            DispatchDeclarationOwner::Accessor(id)
                        }
                        _ => return Err(Error::Application(owner)),
                    }
                }
                _ => return Err(Error::Application(owner)),
            };
            mir::MirDispatchImplementationV1::AbstractObligation {
                declaration,
                trap_target: target,
                receiver,
            }
        }
        mir::MirCallableLoweringRoleV1::DispatchAdjust { .. }
        | mir::MirCallableLoweringRoleV1::BoxingAdjust { .. } => {
            mir::MirDispatchImplementationV1::AdjustThunkTarget(target)
        }
        _ => {
            let is_default = binding
                .lowered_signature()
                .exact()
                .receiver()
                .into_option()
                .and_then(|exact| context.authority.types.get(exact))
                .is_some_and(|ty| {
                    matches!(ty.representation(), mir::MirTypeRepresentationV1::Interface)
                });
            if is_default {
                mir::MirDispatchImplementationV1::InterfaceDefaultTarget { target, receiver }
            } else {
                mir::MirDispatchImplementationV1::DirectStrongTarget { target, receiver }
            }
        }
    };
    Ok(mir::MirDispatchEntryV1::from_contract(
        contract,
        implementation,
    ))
}
