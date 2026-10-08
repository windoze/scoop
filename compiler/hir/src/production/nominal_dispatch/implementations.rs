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
            let role = self.interface_role(implementation.interface)?;
            for method in &implementation.methods {
                let slot = self.interface_slot(method.member)?;
                let selection = match method.target {
                    InterfaceImplementationTarget::DerivedEquality(application) => self
                        .derived_selection(
                            self.export.derived_equality_applications[application].owner_ty,
                        )?,
                    InterfaceImplementationTarget::ImportedDerivedEquality(_) => {
                        self.derived_selection(host)?
                    }
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
