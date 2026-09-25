use super::*;

impl Projection<'_> {
    pub(super) fn interface_slot(
        &self,
        member: InterfaceMethodId,
    ) -> Result<PersistentDispatchSlotId, Error> {
        self.export
            .dispatch_slot_identities
            .get_interface(member)
            .map(|identity| identity.id())
            .ok_or_else(|| invalid("interface member has no sealed dispatch identity"))
    }

    pub(in crate::production) fn interface_members(
        &mut self,
        application: InterfaceApplicationId,
    ) -> Result<Vec<InterfaceMethodId>, Error> {
        let mut members = Vec::new();
        let mut suppressed = BTreeSet::new();
        for application in self.interface_postorder(application)? {
            let interface = self.export.interface_applications[application].template;
            for member in &self.export.interfaces[interface].methods {
                self.push(&mut members, *member)?;
                for overridden in &self.export.interface_methods[*member].overrides {
                    suppressed.insert(*overridden);
                }
            }
        }
        let mut effective = Vec::new();
        for member in members {
            if !suppressed.contains(&member) {
                self.push(&mut effective, member)?;
            }
        }
        Ok(effective)
    }

    pub(in crate::production) fn interface_postorder(
        &mut self,
        application: InterfaceApplicationId,
    ) -> Result<Vec<InterfaceApplicationId>, Error> {
        let mut result = Vec::new();
        let mut complete = BTreeSet::new();
        let mut active = BTreeSet::new();
        let mut pending = Vec::new();
        self.push(&mut pending, (application, false, 1))?;
        while let Some((application, leaving, depth)) = pending.pop() {
            if complete.contains(&application) {
                continue;
            }
            let source = &self.export.interface_applications[application];

            if leaving {
                active.remove(&application);
                complete.insert(application);
                self.push(&mut result, application)?;
                continue;
            }
            if !active.insert(application) {
                return Err(invalid("cycle in source interface inheritance"));
            }
            self.push(&mut pending, (application, true, depth))?;
            for parent in self.export.interfaces[source.template].parents.iter().rev() {
                self.push(&mut pending, (*parent, false, depth + 1))?;
            }
        }
        Ok(result)
    }
}
