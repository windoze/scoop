//! Complete scalar inputs needed by conditional container materialization.

use super::*;

impl Lowerer {
    pub(crate) fn prepare_imported_encoding_inputs(
        &mut self,
    ) -> Result<(), ImportedSignatureTypeError> {
        let interface = self
            .loaded_class_definitions
            .values()
            .find_map(|loaded| loaded.definition.element_encoding.as_ref())
            .or_else(|| {
                self.loaded_enum_definitions
                    .values()
                    .find_map(|loaded| loaded.definition.element_encoding.as_ref())
            })
            .map(|encoding| encoding.implementation.interface);
        let Some(interface) = interface else {
            return Ok(());
        };
        self.require_imported_bound_interface(interface)?;

        // A scalar may reach a container through a generic function without
        // any direct scalar member call in this Cone. Retain declarations for
        // those actual arguments before the checked export graph is sealed.
        let mut arguments = self
            .class_applications
            .values()
            .flat_map(|value| &value.arguments)
            .chain(
                self.enum_applications
                    .values()
                    .flat_map(|value| &value.arguments),
            )
            .chain(
                self.struct_applications
                    .values()
                    .flat_map(|value| &value.arguments),
            )
            .chain(
                self.interface_applications
                    .values()
                    .flat_map(|value| &value.arguments),
            )
            .chain(
                self.instantiations
                    .values()
                    .flat_map(|value| &value.type_args),
            )
            .chain(
                self.generic_method_applications
                    .values()
                    .flat_map(|value| value.method_arguments.iter()),
            )
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        for application in self.imported_generic_applications.values() {
            let values = match &application.arguments {
                hir::ImportedCallableArguments::Function(values) => values,
                hir::ImportedCallableArguments::Method {
                    method_arguments, ..
                } => method_arguments,
            };
            arguments.extend(values.iter().copied());
        }
        for argument in arguments {
            self.resolve_imported_member_receiver_type(argument)?;
        }
        Ok(())
    }
}
