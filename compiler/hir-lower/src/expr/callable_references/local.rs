use super::*;

impl Lowerer {
    pub(super) fn lower_local_callable_reference(
        &mut self,
        candidates: Vec<hir::LocalFunctionId>,
        name: &ast::Ident,
        span: Span,
        expected: Option<TypeId>,
    ) -> Result<Option<hir::Expr>, ()> {
        let expected_signature = self.expected_function_signature(expected);
        let owner_type_param_count = self.signatures
            [&self.local_functions[candidates[0]].source_function()]
            .owner_type_param_count;
        let owner_type_args = self.ambient_type_args(owner_type_param_count);
        let functions = candidates
            .iter()
            .map(|candidate| self.local_functions[*candidate].source_function())
            .collect::<Vec<_>>();
        let display = format!("local callable reference `::{}`", name.text);
        let resolved = match self.resolve_reference_candidates(
            &functions,
            &owner_type_args,
            ReferenceResolutionContext {
                expected: expected_signature.as_ref(),
                name: &name.text,
                display: &display,
                span,
                extension_mode: ReferenceExtensionMode::Exclude,
            },
        ) {
            ReferenceResolutionOutcome::Resolved(resolved) => resolved,
            ReferenceResolutionOutcome::NoApplicable => return Ok(None),
            ReferenceResolutionOutcome::Blocked => return Ok(None),
            ReferenceResolutionOutcome::Failed => return Err(()),
        };
        let ReferenceTarget::Callable(hir::CallableReferenceTarget::Named(
            hir::CallableTarget::Local(callee),
        )) = resolved.target
        else {
            unreachable!("lexical reference candidates are local named declarations")
        };
        let function = self.callable_function_id(callee);
        let local_function = self.local_function_by_function[&function];
        let ty = resolved.ty;
        let Type::Function(function_type) = self.types[ty] else {
            unreachable!("a callable reference has a function type")
        };
        let Some(capture_sources) = self.local_call_capture_args(local_function, span) else {
            return Err(());
        };
        let capture_specs: Vec<_> = self.local_functions[local_function]
            .captures
            .iter()
            .map(|capture| {
                (
                    capture.binding,
                    capture.name.clone(),
                    capture.ty,
                    capture.first_use_span,
                )
            })
            .collect();
        let captures = capture_specs
            .into_iter()
            .zip(capture_sources)
            .map(
                |((binding, name, ty, first_use_span), source)| hir::Capture {
                    binding,
                    name,
                    ty: self.instantiate_ty(ty, &resolved.type_args),
                    first_use_span,
                    source,
                },
            )
            .collect();
        let definition_path = self
            .definition_paths
            .next(scoop_identity::StructuralDefinitionSiteRole::CallableConversion);
        let owner_type_arguments = self.ambient_type_args(self.type_params_in_scope.len());
        let id = self.callable_references.alloc(hir::CallableReference {
            definition_root: hir::CallableReferenceRoot::Source(self.current_definition_root()),
            definition_path,
            target: hir::CallableReferenceTarget::Local {
                definition_path: self.local_functions[local_function].definition_path.clone(),
                callee: callee.into(),
            },
            function_type,
            owner_type_arguments,
            captures,
            origin: self.definition_origin(span),
            span,
        });
        Ok(Some(hir::Expr {
            kind: ExprKind::CallableReference(id),
            ty,
            span,
            origin: self.expression_origin(span),
        }))
    }
}
