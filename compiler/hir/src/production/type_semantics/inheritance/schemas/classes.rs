use super::*;
use crate::production::nominal_dispatch::ClassChainEntry;

impl Projection<'_> {
    pub(super) fn class(&mut self, class: ClassId) -> Result<InheritanceSlotSchemaV1, Error> {
        let chain = self.class_chain(class)?;
        let mut slots = Vec::new();
        let mut seen = BTreeSet::new();
        for entry in chain.into_iter().rev() {
            let class = match entry {
                ClassChainEntry::Local(class) => class,
                ClassChainEntry::Imported(ty) => {
                    let Type::Class(application) = self.export.types[ty] else {
                        return Err(self.invalid("class base does not resolve to a class type"));
                    };
                    let class = &self.export.loaded_class_definitions
                        [&self.export.class_applications[application].template];
                    for method in &class.virtual_methods {
                        if seen.insert(method.slot) {
                            self.push(&mut slots, method.slot)?;
                        }
                    }
                    continue;
                }
            };
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

                let identity = self
                    .export
                    .dispatch_slot_identities
                    .get_virtual(family)
                    .ok_or_else(|| {
                        self.invalid("virtual family has no sealed dispatch identity")
                    })?;
                if seen.insert(identity.id()) {
                    self.push(&mut slots, identity.id())?;
                }
            }
        }
        self.schema(InheritanceSlotSchemaRoleV1::ClassVtable, slots)
    }

    pub(super) fn class_chain(&mut self, class: ClassId) -> Result<Vec<ClassChainEntry>, Error> {
        let result =
            crate::production::nominal_dispatch::Projection::new(self.export).class_chain(class)?;
        for entry in &result {
            let base = match entry {
                ClassChainEntry::Local(class) => self.export.classes[*class].base_class,
                ClassChainEntry::Imported(ty) => Some(*ty),
            };
            if let Some(base) = base {
                exact(self.export, base)?;
            }
        }
        Ok(result)
    }
}
