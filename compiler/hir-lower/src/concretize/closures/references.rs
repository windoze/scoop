use super::*;

impl Concretizer<'_> {
    pub(in crate::concretize) fn lower_imported_reference(
        &mut self,
        source: &export::ImportedCallableReference,
        span: scoop_ast::Span,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::CallableReferenceId {
        let function_type = self.lower_function_type(source.function_type, substitution);
        let target = match &source.target {
            export::ImportedCallableReferenceTarget::Named(callee) => {
                concrete::CallableReferenceTarget::Named(
                    self.lower_imported_callable_target(*callee, substitution),
                )
            }
            export::ImportedCallableReferenceTarget::Local(application) => {
                let application = self.source.imported_generic_applications[*application].clone();
                let template = &self.source.imported_generic_templates[application.template];
                let export::ImportedCallableTemplateOrigin::Local { descriptor, .. } =
                    &template.declaration
                else {
                    unreachable!("a local reference retains its source local declaration");
                };
                let path = descriptor.definition_path().clone();
                let function = self.lower_imported_callable_application(&application, substitution);
                let local_function = self.local_functions.alloc(concrete::LocalFunction {
                    definition_path: path,
                    function,
                    function_type,
                    span,
                });
                concrete::CallableReferenceTarget::Local {
                    local_function,
                    callee: concrete::Callable::Function(function),
                }
            }
            export::ImportedCallableReferenceTarget::BoundMember { receiver, callee } => {
                concrete::CallableReferenceTarget::BoundMember {
                    receiver: Box::new(self.lower_expr(receiver, substitution, locals)),
                    callee: self.lower_imported_callable_target(*callee, substitution),
                }
            }
            export::ImportedCallableReferenceTarget::BoundExtension { receiver, callee } => {
                concrete::CallableReferenceTarget::BoundExtension {
                    receiver: Box::new(self.lower_expr(receiver, substitution, locals)),
                    callee: self.lower_imported_callable_target(*callee, substitution),
                }
            }
        };
        let pending = PendingCallableReference {
            source: CallableReferenceSource::Imported {
                parent: source.parent,
                definition: source.definition.clone(),
            },
            owner_arguments: source
                .owner_type_arguments
                .iter()
                .map(|ty| self.lower_type(*ty, substitution))
                .collect(),
            target,
            function_type,
            captures: source
                .captures
                .iter()
                .map(|capture| self.lower_capture(capture, substitution, locals))
                .collect(),
            origin: source.origin,
            span,
        };
        let id = concrete::CallableReferenceId::from_raw(
            (self.callable_reference_slots.len() as u32).into(),
        );
        self.callable_reference_slots.push(pending);
        id
    }

    pub(in crate::concretize) fn lower_imported_callable_target(
        &mut self,
        callee: export::ImportedCallableTarget,
        substitution: &[concrete::TypeId],
    ) -> concrete::CallableTarget {
        match callee {
            export::ImportedCallableTarget::Application(application) => {
                let application = self.source.imported_generic_applications[application].clone();
                concrete::CallableTarget::Local(concrete::Callable::Function(
                    self.lower_imported_callable_application(&application, substitution),
                ))
            }
            export::ImportedCallableTarget::Dependency(callee) => {
                concrete::CallableTarget::Imported(self.imported_dependency_callable_map[&callee])
            }
        }
    }
}
