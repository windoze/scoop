use super::*;

mod merge;

impl Projection<'_> {
    pub(super) fn selections(&mut self, nominal: NominalOwner) -> Result<Selections, Error> {
        let mut selections = Selections::new();
        let (implementations, allow_abstract) = match nominal {
            NominalOwner::Class(id) => {
                self.virtual_selections(id, &mut selections)?;
                let class = &self.export.classes[id];
                (
                    &class.interface_implementations,
                    class.modifier == ClassModifier::Abstract,
                )
            }
            NominalOwner::Object(id) => {
                let class = self.export.objects[id].backing_class;
                self.virtual_selections(class, &mut selections)?;
                (&self.export.classes[class].interface_implementations, false)
            }
            NominalOwner::Struct(id) => (&self.export.structs[id].interface_implementations, false),
            NominalOwner::Enum(id) => (&self.export.enums[id].interface_implementations, false),
            NominalOwner::Interface(id) => {
                for member in self.interface_members(self.export.interfaces[id].self_application)? {
                    let slot = self.interface_slot(member)?;
                    let selection = self.interface_selection(member)?;
                    self.merge_selection(&mut selections, slot, selection)?;
                }
                return Ok(selections);
            }
        };
        for implementation in implementations {
            for method in &implementation.methods {
                let slot = self.interface_slot(method.member)?;
                let selection = match method.target {
                    InterfaceImplementationTarget::Method(application) => {
                        self.target(application)?
                    }
                    InterfaceImplementationTarget::Imported(callable) => {
                        Selection::InterfaceDefault(self.imported_callable(callable)?)
                    }
                    InterfaceImplementationTarget::ImportedAbstract(_)
                    | InterfaceImplementationTarget::Subclass
                        if allow_abstract =>
                    {
                        Selection::Abstract
                    }
                    InterfaceImplementationTarget::ImportedAbstract(_)
                    | InterfaceImplementationTarget::Subclass => {
                        return Err(invalid(
                            "non-abstract owner leaves an interface slot abstract",
                        ));
                    }
                };
                self.merge_selection(&mut selections, slot, selection)?;
            }
        }
        Ok(selections)
    }

    fn virtual_selections(
        &mut self,
        class: ClassId,
        selections: &mut Selections,
    ) -> Result<(), Error> {
        // The first encounter in derived-to-base order is the selected override.
        for class in self.class_chain(class)? {
            for function in &self.export.classes[class].methods {
                let method = self.method(*function)?;
                let family = match method.dispatch {
                    MethodDispatch::Virtual(family) | MethodDispatch::FinalOverride(family) => {
                        family
                    }
                    MethodDispatch::Direct => continue,
                    MethodDispatch::Interface(_) => {
                        return Err(invalid(
                            "class method carries interface declaration dispatch",
                        ));
                    }
                };
                let slot = self
                    .export
                    .dispatch_slot_identities
                    .get_virtual(family)
                    .ok_or_else(|| invalid("virtual family has no sealed dispatch identity"))?
                    .id();

                if !selections.contains_key(&slot) {
                    let selection = if method.modifier == MethodModifier::Abstract {
                        Selection::Abstract
                    } else {
                        Selection::Concrete(self.callable(*function)?)
                    };
                    self.merge_selection(selections, slot, selection)?;
                }
            }
        }
        Ok(())
    }

    fn merge_selection(
        &mut self,
        selections: &mut Selections,
        slot: PersistentDispatchSlotId,
        selection: Selection,
    ) -> Result<(), Error> {
        merge::insert(selections, slot, selection)
    }
}
