use super::*;

impl Context<'_> {
    pub(super) fn entries(
        &self,
        local: &hir::LocalConcreteHir,
        owner: PersistentExactTypeId,
        schema: &hir::InheritanceSlotSchemaV1,
        targets: &[CallableDefinitionOwner],
    ) -> Result<Vec<mir::MirDispatchEntryV1>, Error> {
        let mut entries = reserve(schema.slots().len())?;
        for (position, (slot, target)) in schema.slots().iter().zip(targets).enumerate() {
            let contract = self
                .source
                .inheritance()
                .get(owner)
                .and_then(|record| record.slots().get(*slot))
                .ok_or(Error::MissingSelection { owner, slot: *slot })?;
            let exact = contract.signature().exact_signature();
            let receiver = match schema.role() {
                hir::InheritanceSlotSchemaRoleV1::ClassVtable => exact.receiver().into_option(),
                hir::InheritanceSlotSchemaRoleV1::Interface { interface_exact } => {
                    Some(interface_exact)
                }
            };
            let mut parameters = reserve(exact.parameters().len())?;
            parameters.extend_from_slice(exact.parameters());

            let signature = mir::MirBridgeCallableSignatureV1::new(
                crate::coroutine_registry::lowered_signature(
                    local,
                    &ExactCallableSignature::new(
                        exact.effect(),
                        receiver,
                        parameters,
                        exact.result(),
                    ),
                ),
                match contract.signature().effects().gc_effect() {
                    scoop_identity::GcEffect::Managed => mir::GcEffect::Managed,
                    scoop_identity::GcEffect::NoGc => mir::GcEffect::NoGc,
                },
            );
            let contract = mir::MirDispatchSlotV1::new(
                *slot,
                mir::MirDispatchPositionV1::new(
                    u32::try_from(position)
                        .map_err(|_| Error::TargetMismatch { owner, slot: *slot })?,
                ),
                signature,
            );
            entries.push(applications::entry(self, owner, contract, *target)?);
        }
        Ok(entries)
    }
}
