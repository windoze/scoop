use super::*;

pub(super) fn interface(
    local: &hir::LocalConcreteHir,
    interface: PersistentExactTypeId,
) -> Result<Vec<mir::MirDispatchSlotV1>, Error> {
    let ty = local
        .exact_type_identities
        .type_for_identity(interface)
        .ok_or(Error::Application(interface))?;
    let hir::concrete::TypeKind::Interface(id) = local.types[ty].kind else {
        return Err(Error::Application(interface));
    };
    let mut contracts = reserve(local.interfaces[id].methods.len())?;
    for ((position, slot), method) in local
        .dispatch_slot_identities
        .interface_slots(id)
        .zip(&local.interfaces[id].methods)
    {
        let signature = ExactCallableSignature::new(
            if method.is_suspend {
                scoop_identity::Effect::Suspend
            } else {
                scoop_identity::Effect::Ordinary
            },
            Some(interface),
            method
                .params
                .iter()
                .map(|parameter| local.exact_type_identities[parameter.ty].id())
                .collect(),
            local.exact_type_identities[method.return_ty].id(),
        );
        contracts.push(mir::MirDispatchSlotV1::new(
            slot.id(),
            mir::MirDispatchPositionV1::new(position.into_raw()),
            mir::MirBridgeCallableSignatureV1::new(
                signature,
                crate::lowering_support::lower_gc_effect(method.attributes.gc_effect),
            ),
        ));
    }
    Ok(contracts)
}
