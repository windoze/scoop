use super::*;

impl Projection<'_, '_> {
    pub(super) fn class(
        &mut self,
        class: ClassId,
    ) -> Result<(InheritanceSlotSchemaV1, Vec<TypeId>), Error> {
        let chain = self.class_chain(class)?;
        let mut slots = Vec::new();
        let mut seen = BTreeSet::new();
        let mut interfaces = Vec::new();
        for class in chain.into_iter().rev() {
            let declaration = &self.export.classes[class];
            self.extend(&mut interfaces, &declaration.interfaces)?;
            for function in &declaration.methods {
                self.work(1)?;
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
                self.search(seen.len())?;
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
        Ok((
            self.schema(InheritanceSlotSchemaRoleV1::ClassVtable, slots)?,
            interfaces,
        ))
    }

    pub(super) fn class_chain(&mut self, class: ClassId) -> Result<Vec<ClassId>, Error> {
        let mut result = Vec::new();
        let mut seen = BTreeSet::new();
        let mut current = class;
        loop {
            self.depth(result.len() + 1)?;
            self.search(seen.len())?;
            if !seen.insert(current) {
                return Err(self.invalid("cycle in the source class base chain"));
            }
            self.push(&mut result, current)?;
            let Some(base) = self.export.classes[current].base_class else {
                return Ok(result);
            };
            exact(self.export, base)?;
            let Type::Class(application) = self.export.types[base] else {
                return Err(self.invalid("class base does not resolve to a class application"));
            };
            current = self.export.class_applications[application].template;
        }
    }
}
