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
                        self.imported_selection(callable)?
                    }
                    InterfaceImplementationTarget::ImportedTemplate(application) => {
                        self.imported_template_selection(application)?
                    }
                    InterfaceImplementationTarget::Abstract(application) if allow_abstract => {
                        let application = &self.export.method_applications[application];
                        Selection::Abstract(self.callable(application.function)?)
                    }
                    InterfaceImplementationTarget::ImportedAbstract(callable) if allow_abstract => {
                        Selection::Abstract(self.imported_callable(callable)?)
                    }
                    InterfaceImplementationTarget::ImportedAbstractTemplate(application)
                        if allow_abstract =>
                    {
                        Selection::Abstract(
                            self.imported_template_selection(application)?.declaration(),
                        )
                    }
                    InterfaceImplementationTarget::ImportedAbstract(_)
                    | InterfaceImplementationTarget::ImportedAbstractTemplate(_)
                    | InterfaceImplementationTarget::Abstract(_) => {
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
        for entry in self.class_chain(class)? {
            let class = match entry {
                ClassChainEntry::Local(class) => class,
                ClassChainEntry::Imported(ty) => {
                    let Type::ImportedClass(class) = &self.export.types[ty] else {
                        return Err(invalid("class base does not resolve to a class type"));
                    };
                    for selection in class
                        .declaration
                        .interface
                        .declaration_details()
                        .dispatch_selections()
                        .records()
                    {
                        if class
                            .virtual_methods
                            .iter()
                            .any(|method| method.slot == selection.slot())
                            && !selections.contains_key(&selection.slot())
                        {
                            self.merge_selection(
                                selections,
                                selection.slot(),
                                selection.selection(),
                            )?;
                        }
                    }
                    continue;
                }
            };
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
                        Selection::Abstract(self.callable(*function)?)
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
