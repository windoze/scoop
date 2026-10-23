use super::*;

impl Projection<'_> {
    pub(super) fn implementation_selections(
        &mut self,
        implementations: &[InterfaceImplementation],
        allow_abstract: bool,
        host: TypeId,
        selections: &mut Selections,
    ) -> Result<(), Error> {
        for implementation in implementations {
            let role = if let Some(condition) = &implementation.condition {
                SelectionRole::ConditionalInterface {
                    interface: self.type_key(implementation.interface)?,
                    condition: ConditionalInterfaceApplicationV1 {
                        declaration: condition.declaration,
                        owner: match condition.owner {
                            SourceNominalId::Concrete(owner) => {
                                scoop_identity::SignatureTypeKey::Nominal(owner)
                            }
                            SourceNominalId::GenericTemplate(origin) => {
                                scoop_identity::SignatureTypeKey::NominalApplication {
                                    origin,
                                    arguments: scoop_identity::NonEmptyVec::new(
                                        condition
                                            .arguments
                                            .iter()
                                            .map(|&argument| self.type_key(argument))
                                            .collect::<Result<_, _>>()?,
                                    )
                                    .map_err(|_| {
                                        invalid(
                                            "conditional owner is missing its original arguments",
                                        )
                                    })?,
                                }
                            }
                        },
                    },
                }
            } else {
                self.interface_role(implementation.interface)?
            };
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
                let receiver = self.type_key(self.selected_receiver(method.target, host)?)?;
                self.merge_selection(selections, role.clone(), receiver, slot, selection)?;
            }
        }
        Ok(())
    }
}
