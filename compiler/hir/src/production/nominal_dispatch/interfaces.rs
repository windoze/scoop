use super::*;

impl Projection<'_> {
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
                .ok_or_else(|| invalid("interface member has no dispatch identity")),
            InterfaceMethodReference::Imported { slot, .. } => Ok(slot),
        }
    }

    pub(in crate::production) fn interface_members(
        &mut self,
        application: InterfaceApplicationId,
    ) -> Result<Vec<InterfaceMethodReference>, Error> {
        let ty = self.export.interface_applications[application].canonical_type;
        let mut members = Vec::new();
        let mut suppressed = BTreeSet::new();
        for ty in self.interface_postorder(ty)? {
            match &self.export.types[ty] {
                Type::Interface(application) => {
                    let owner = self
                        .export
                        .nominal_identities
                        .interface_id(self.export.interface_applications[*application].template)
                        .expect("an interface application retains its declaration");
                    for member in &self.export.interfaces[owner].methods {
                        self.push(&mut members, InterfaceMethodReference::Local(*member))?;
                        for reference in &self.export.interface_methods[*member].overrides {
                            suppressed.insert(self.interface_slot(*reference)?);
                        }
                    }
                }
                Type::ImportedInterface(interface) => {
                    let owner = PublicDeclarationOwnerV1::Nominal(interface.declaration.owner());
                    for method in &interface.methods {
                        if method.declaration.owner() == owner {
                            self.push(
                                &mut members,
                                InterfaceMethodReference::Imported {
                                    owner: ty,
                                    slot: method.slot.id(),
                                },
                            )?;
                            suppressed.extend(method.overrides.iter().copied());
                        }
                    }
                }
                _ => return Err(invalid("interface parent has a non-interface type")),
            }
        }
        let mut effective = Vec::new();
        for member in members {
            if !suppressed.contains(&self.interface_slot(member)?) {
                self.push(&mut effective, member)?;
            }
        }
        Ok(effective)
    }

    fn interface_postorder(&mut self, ty: TypeId) -> Result<Vec<TypeId>, Error> {
        let mut result = Vec::new();
        let mut seen = BTreeSet::new();
        let mut pending = Vec::new();
        self.push(&mut pending, (ty, false))?;
        while let Some((ty, leaving)) = pending.pop() {
            if leaving {
                self.push(&mut result, ty)?;
                continue;
            }
            if !seen.insert(ty) {
                continue;
            }
            let parents = match &self.export.types[ty] {
                Type::Interface(application) => {
                    let owner = self
                        .export
                        .nominal_identities
                        .interface_id(self.export.interface_applications[*application].template)
                        .expect("an interface application retains its declaration");
                    &self.export.interfaces[owner].parents
                }
                Type::ImportedInterface(interface) => &interface.parents,
                _ => return Err(invalid("interface parent has a non-interface type")),
            };
            self.push(&mut pending, (ty, true))?;
            for parent in parents.iter().rev() {
                self.push(&mut pending, (*parent, false))?;
            }
        }
        Ok(result)
    }
}
