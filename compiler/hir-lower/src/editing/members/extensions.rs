use super::*;
use crate::call_resolution::applicability::{
    DeclarationApplicabilityInput, DeclarationTypeArguments,
};
use crate::imports::lookup::calls::{ExtensionCallTarget, ExtensionPropertyTarget};
use hir::ImportedCallableSource;

impl Lowerer {
    pub(super) fn editing_extensions(
        &mut self,
        receiver: hir::TypeId,
        members: &mut hir::EditingMembers,
        is_cancelled: &impl Fn() -> bool,
    ) -> Result<(), EditingCancelled> {
        for name in self.editing_lookup_names() {
            super::super::receivers::check_cancelled(is_cancelled)?;
            for layer in self.named_executable_extension_call_layers(&name) {
                if !layer.unavailable.is_empty() || !layer.suppressed_callables.is_empty() {
                    unavailable(members, name.clone(), U::SignatureFailure);
                    break;
                }
                for target in layer.candidates {
                    let candidate = match target {
                        ExtensionCallTarget::Current(function) => self
                            .editing_source_extension_applicable(function, receiver)
                            .map(|applies| {
                                applies.then_some(Target::Callable(hir::SemanticCallable::Current(
                                    function,
                                )))
                            }),
                        ExtensionCallTarget::Dependency(binding) => self
                            .dependencies
                            .as_ref()
                            .and_then(|dependencies| dependencies.callable_candidate(&binding).ok())
                            .and_then(|candidate| {
                                self.dependencies
                                    .as_ref()?
                                    .callable_declaration(candidate.interface().declaration())
                                    .ok()
                            })
                            .ok_or(U::DeclarationFailure)
                            .and_then(|declaration| {
                                self.editing_imported_extension_applicable(&declaration, receiver)
                                    .map(|applies| {
                                        applies
                                            .then(|| {
                                                imported::callable(
                                                    declaration.interface().declaration(),
                                                )
                                                .map(Target::Callable)
                                            })
                                            .flatten()
                                    })
                            }),
                    };
                    record(members, &name, candidate);
                }
            }
            for layer in self.named_extension_property_layers(&name) {
                if !layer.unavailable.is_empty() {
                    unavailable(members, name.clone(), U::SignatureFailure);
                    break;
                }
                for target in layer.candidates {
                    let candidate = match target {
                        ExtensionPropertyTarget::Current(property) => {
                            if !self.property_is_accessible(property, Some(receiver)) {
                                continue;
                            }
                            self.editing_source_extension_applicable(
                                self.extension_property_getter(property),
                                receiver,
                            )
                            .map(|applies| applies.then_some(Target::Property(property)))
                        }
                        ExtensionPropertyTarget::Dependency(binding) => {
                            self.editing_imported_extension_property(&binding, receiver)
                        }
                    };
                    record(members, &name, candidate);
                }
            }
        }
        Ok(())
    }

    fn editing_source_extension_applicable(
        &mut self,
        function: hir::FunctionId,
        receiver: hir::TypeId,
    ) -> Result<bool, U> {
        if self
            .declaration_surface
            .unavailable_signature(function)
            .is_some()
            || self.declaration_surface.rejects_function(function)
        {
            return Err(U::SignatureFailure);
        }
        let parameters = self.signatures[&function].type_params.clone();
        let declared = self.extension_receivers[&function];
        Ok(self.editing_receiver_applicable(&parameters, declared, receiver))
    }

    pub(super) fn editing_receiver_applicable(
        &mut self,
        parameters: &[hir::TypeParamDecl],
        declared: hir::TypeId,
        receiver: hir::TypeId,
    ) -> bool {
        let (session, _) = self.declaration_applicability_session(DeclarationApplicabilityInput {
            availability: None,
            owner_parameters: &[],
            callable_parameters: parameters,
            type_arguments: DeclarationTypeArguments::Callable {
                owner_arguments: &[],
            },
            explicit_arguments: &[],
            bound_receiver: Some((declared, receiver)),
        });
        self.constraints_are_satisfiable(&session)
    }

    fn editing_imported_extension_property(
        &mut self,
        binding: &hir::DirectImportedTargetBinding,
        receiver: hir::TypeId,
    ) -> Result<Option<Target>, U> {
        let dependencies = self.dependencies.as_ref().ok_or(U::DeclarationFailure)?;
        let property = dependencies
            .property_candidate(binding)
            .map_err(|_| U::DeclarationFailure)?;
        let getter = dependencies
            .callable_declaration(scoop_identity::CallableTemplateOrigin::Accessor(
                property.interface().accessors().getter(),
            ))
            .map_err(|_| U::DeclarationFailure)?;
        self.editing_imported_extension_applicable(&getter, receiver)
            .map(|applies| {
                applies.then_some(Target::ImportedProperty(property.interface().declaration()))
            })
    }
}

fn record(members: &mut hir::EditingMembers, name: &str, candidate: Result<Option<Target>, U>) {
    match candidate {
        Ok(Some(target)) => add(members, name.to_owned(), target, true),
        Ok(None) => {}
        Err(reason) => unavailable(members, name.to_owned(), reason),
    }
}
