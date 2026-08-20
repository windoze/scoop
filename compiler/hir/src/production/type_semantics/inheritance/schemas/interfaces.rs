use super::*;

impl Projection<'_> {
    pub(super) fn interface(
        &mut self,
        application: InterfaceApplicationId,
    ) -> Result<InheritanceSlotSchemaV1, Error> {
        let interface_exact = exact(
            self.export,
            self.export.interface_applications[application].canonical_type,
        )?;
        let mut slots = Vec::new();
        for member in self.interface_members(application)? {
            let slot = self.interface_slot(member)?;
            self.push(&mut slots, slot)?;
        }
        self.schema(
            InheritanceSlotSchemaRoleV1::Interface { interface_exact },
            slots,
        )
    }

    pub(super) fn interface_slot(
        &self,
        member: InterfaceMethodReference,
    ) -> Result<PersistentDispatchSlotId, Error> {
        match member {
            InterfaceMethodReference::Local(member) => self
                .export
                .dispatch_slot_identities
                .get_interface(member)
                .map(|identity| identity.id())
                .ok_or_else(|| self.invalid("interface member has no dispatch identity")),
            InterfaceMethodReference::Imported { slot, .. } => Ok(slot),
        }
    }

    pub(super) fn interface_members(
        &mut self,
        application: InterfaceApplicationId,
    ) -> Result<Vec<InterfaceMethodReference>, Error> {
        crate::production::nominal_dispatch::Projection::new(self.export)
            .interface_members(application)
    }
}
