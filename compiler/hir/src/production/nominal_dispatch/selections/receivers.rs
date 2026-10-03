//! Retain the receiver that inheritance checking selected for interface code.

use super::*;

impl Projection<'_> {
    pub(super) fn selected_receiver(
        &self,
        target: InterfaceImplementationTarget,
        host: TypeId,
    ) -> Result<TypeId, Error> {
        let candidate = match target {
            InterfaceImplementationTarget::Method(application)
            | InterfaceImplementationTarget::Abstract(application) => {
                match self.export.method_applications[application].owner {
                    MethodOwnerApplication::Interface(application) => {
                        self.export.interface_applications[application].canonical_type
                    }
                    _ => host,
                }
            }
            InterfaceImplementationTarget::Imported(callable)
            | InterfaceImplementationTarget::ImportedAbstract(callable) => {
                self.export.imported_dependency_callables[callable]
                    .receiver()
                    .unwrap_or(host)
            }
            InterfaceImplementationTarget::ImportedTemplate(application)
            | InterfaceImplementationTarget::ImportedAbstractTemplate(application) => {
                match self.export.imported_generic_applications[application].arguments {
                    ImportedCallableArguments::Method { owner, .. } => owner,
                    ImportedCallableArguments::Function(_) => {
                        return Err(invalid(
                            "interface implementation selects a free generic function",
                        ));
                    }
                }
            }
        };
        Ok(
            if matches!(self.export.types[candidate], Type::Interface(_)) {
                candidate
            } else {
                host
            },
        )
    }
}
