use super::*;

impl Projection<'_> {
    pub(super) fn interface_application(
        &self,
        ty: TypeId,
    ) -> Result<InterfaceApplicationId, Error> {
        exact(self.export, ty)?;
        match self.export.types[ty] {
            Type::Interface(application) => Ok(application),
            _ => Err(self.invalid("interface edge does not resolve to an interface application")),
        }
    }

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
        member: InterfaceMethodId,
    ) -> Result<PersistentDispatchSlotId, Error> {
        self.export
            .dispatch_slot_identities
            .get_interface(member)
            .map(|identity| identity.id())
            .ok_or_else(|| self.invalid("interface member has no sealed dispatch identity"))
    }

    pub(super) fn interface_members(
        &mut self,
        application: InterfaceApplicationId,
    ) -> Result<Vec<InterfaceMethodId>, Error> {
        crate::production::nominal_dispatch::Projection::new(self.export)
            .interface_members(application)
    }

    pub(super) fn interface_postorder(
        &mut self,
        application: InterfaceApplicationId,
    ) -> Result<Vec<InterfaceApplicationId>, Error> {
        let result = crate::production::nominal_dispatch::Projection::new(self.export)
            .interface_postorder(application)?;
        for application in &result {
            exact(
                self.export,
                self.export.interface_applications[*application].canonical_type,
            )?;
        }
        Ok(result)
    }
}
