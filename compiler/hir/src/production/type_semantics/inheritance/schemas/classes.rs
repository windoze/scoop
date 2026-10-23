use super::*;

impl Projection<'_> {
    pub(super) fn class(&mut self, source: ClassId) -> Result<InheritanceSlotSchemaV1, Error> {
        let origin = &self.export.nominal_identities[source];
        let (class, _) = self
            .local
            .classes
            .iter()
            .find(|(_, class)| &class.origin == origin && class.type_arguments.is_empty())
            .ok_or_else(|| self.invalid("concrete source class has no materialized definition"))?;
        let mut chain = Vec::new();
        let mut current = Some(class);
        while let Some(class) = current {
            chain.push(class);
            current = self.local.classes[class].base_class();
        }
        let mut slots = Vec::new();
        let mut seen = BTreeSet::new();
        // Concretization has already removed unavailable members. Reuse its
        // family order, including unconditional overrides of conditional slots.
        for class in chain.into_iter().rev() {
            for method in &self.local.classes[class].methods {
                let family = match *method {
                    concrete::ClassMethod::Imported { family, .. } => family,
                    concrete::ClassMethod::Local(function) => {
                        let method = self.local.functions[function]
                            .receiver
                            .method()
                            .expect("class methods have a checked receiver");
                        match method.dispatch {
                            concrete::MethodDispatch::Virtual(family)
                            | concrete::MethodDispatch::FinalOverride(family) => family,
                            _ => continue,
                        }
                    }
                };
                let slot = self
                    .local
                    .dispatch_slot_identities
                    .virtual_slot(family)
                    .id();
                if seen.insert(slot) {
                    self.push(&mut slots, slot)?;
                }
            }
        }
        self.schema(InheritanceSlotSchemaRoleV1::ClassVtable, slots)
    }
}
