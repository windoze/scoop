use super::*;

impl Projection<'_> {
    pub(super) fn class(&mut self, class: ClassId) -> Result<InheritanceSlotSchemaV1, Error> {
        let chain = self.class_chain(class)?;
        let mut slots = Vec::new();
        let mut seen = BTreeSet::new();
        for class in chain.into_iter().rev() {
            let declaration = &self.export.classes[class];
            for function in &declaration.methods {
                let Some(method) = self.export.functions[*function].method else {
                    return Err(self.invalid("class method has no dispatch metadata"));
                };
                let family = match method.dispatch {
                    MethodDispatch::Virtual(family) | MethodDispatch::FinalOverride(family) => {
                        family
                    }
                    MethodDispatch::Direct => continue,
                    MethodDispatch::Interface(_) => {
                        return Err(
                            self.invalid("class method carries interface declaration dispatch")
                        );
                    }
                };

                if !seen.insert(family) {
                    continue;
                }
                let identity = self
                    .export
                    .dispatch_slot_identities
                    .get_virtual(family)
                    .ok_or_else(|| {
                        self.invalid("virtual family has no sealed dispatch identity")
                    })?;
                self.push(&mut slots, identity.id())?;
            }
        }
        self.schema(InheritanceSlotSchemaRoleV1::ClassVtable, slots)
    }

    pub(super) fn class_chain(&mut self, class: ClassId) -> Result<Vec<ClassId>, Error> {
        let result =
            crate::production::nominal_dispatch::Projection::new(self.export).class_chain(class)?;
        for class in &result {
            if let Some(base) = self.export.classes[*class].base_class {
                exact(self.export, base)?;
            }
        }
        Ok(result)
    }
}
