use super::*;

impl Projection<'_, '_> {
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
        let mut members = Vec::new();
        let mut suppressed = BTreeSet::new();
        for application in self.interface_postorder(application)? {
            let interface = self.export.interface_applications[application].template;
            let declaration = &self.export.interfaces[interface];
            self.extend(&mut members, &declaration.methods)?;
            for member in &declaration.methods {
                for overridden in &self.export.interface_methods[*member].overrides {
                    self.search(suppressed.len())?;
                    suppressed.insert(*overridden);
                }
            }
        }
        let mut effective = Vec::new();
        for member in members {
            self.work(suppressed.len().max(1).ilog2() as usize + 1)?;
            if suppressed.contains(&member) {
                continue;
            }
            self.push(&mut effective, member)?;
        }
        Ok(effective)
    }

    pub(super) fn interface_postorder(
        &mut self,
        application: InterfaceApplicationId,
    ) -> Result<Vec<InterfaceApplicationId>, Error> {
        let mut result = Vec::new();
        let mut complete = BTreeSet::new();
        let mut active = BTreeSet::new();
        let mut pending = Vec::new();
        self.push(&mut pending, (application, false, 1))?;
        while let Some((application, leaving, depth)) = pending.pop() {
            self.depth(depth)?;
            self.search(complete.len())?;
            if complete.contains(&application) {
                continue;
            }
            let source = &self.export.interface_applications[application];
            exact(self.export, source.canonical_type)?;
            self.search(active.len())?;
            if leaving {
                active.remove(&application);
                complete.insert(application);
                self.push(&mut result, application)?;
                continue;
            }
            if !active.insert(application) {
                return Err(self.invalid("cycle in source interface inheritance"));
            }
            self.push(&mut pending, (application, true, depth))?;
            for parent in self.export.interfaces[source.template].parents.iter().rev() {
                self.push(&mut pending, (*parent, false, depth + 1))?;
            }
        }
        Ok(result)
    }
}
