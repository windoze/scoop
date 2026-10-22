use super::*;

impl Lowerer {
    pub(super) fn editing_named_function_candidates(
        &mut self,
        call: &ast::RecoveredCall,
        bindings: &[NamedCallBinding],
        is_cancelled: &impl Fn() -> bool,
    ) -> Result<Candidates, EditingCancelled> {
        let mut candidates = Candidates::default();
        for binding in bindings {
            match (&binding.target, &binding.origin) {
                (NamedCallTarget::Function(function), _) => {
                    if self.extension_receivers.contains_key(function) {
                        for receiver in self.implicit_receivers() {
                            candidates.add(self.editing_source_call_candidate(
                                call,
                                &crate::CallableCandidate::function(*function, Vec::new()),
                                Some(receiver.ty),
                            ));
                        }
                    } else {
                        candidates.add(self.editing_source_call_candidate(
                            call,
                            &crate::CallableCandidate::function(*function, Vec::new()),
                            None,
                        ));
                    }
                }
                (
                    NamedCallTarget::ImportedDependency(
                        hir::ImportedTarget::Function(_) | hir::ImportedTarget::GenericFunction(_),
                    ),
                    NamedCallOrigin::Dependency(binding),
                ) => {
                    let declaration = self.dependencies.as_ref().and_then(|dependencies| {
                        dependencies
                            .callable_candidate(binding)
                            .ok()
                            .and_then(|candidate| {
                                dependencies
                                    .callable_declaration(candidate.interface().declaration())
                                    .ok()
                            })
                    });
                    let Some(declaration) = declaration else {
                        candidates.unavailable = Some(U::DeclarationFailure);
                        continue;
                    };
                    if declaration.interface().owner() == hir::PublicDeclarationOwnerV1::Extension {
                        for receiver in self.implicit_receivers() {
                            candidates.add(self.editing_imported_call_candidate(
                                call,
                                &declaration,
                                Some(receiver.ty),
                            ));
                        }
                    } else {
                        candidates.add(self.editing_imported_call_candidate(
                            call,
                            &declaration,
                            None,
                        ));
                    }
                }
                _ => match self.editing_named_nominal_candidates(binding, call, is_cancelled)? {
                    V::Known(values) => candidates.values.extend(values),
                    V::Unavailable(reason) => {
                        candidates.unavailable.get_or_insert(reason);
                    }
                },
            }
        }
        Ok(candidates)
    }
}
