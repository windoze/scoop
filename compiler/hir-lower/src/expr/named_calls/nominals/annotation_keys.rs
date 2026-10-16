use super::*;

impl Lowerer {
    pub(super) fn lower_annotation_key_query(
        &mut self,
        targets: &[NamedCallBinding],
        call: &ast::CallExpr,
    ) -> Option<Result<hir::Expr, ()>> {
        let binding = targets
            .iter()
            .find(|binding| self.is_annotation_key_query(binding))?;
        let mut state = self.clone();
        let result = state.lower_bound_annotation_key_query(binding, call);
        if result.is_err() && targets.len() != 1 {
            return None;
        }
        *self = state;
        Some(result)
    }

    fn is_annotation_key_query(&self, binding: &NamedCallBinding) -> bool {
        let kind = match (&binding.target, &binding.origin) {
            (NamedCallTarget::Function(id), _) => match &self.functions[*id].kind {
                hir::FunctionKind::Intrinsic(intrinsic) => Some(intrinsic.kind),
                _ => None,
            },
            (NamedCallTarget::ImportedDependency(_), NamedCallOrigin::Dependency(binding)) => self
                .dependencies
                .as_ref()
                .and_then(|world| world.callable_candidate(binding).ok())
                .and_then(
                    |candidate| match candidate.interface().effects().implementation() {
                        hir::CallableImplementationV1::Intrinsic(kind) => Some(kind),
                        _ => None,
                    },
                ),
            _ => None,
        };
        kind == Some(hir::IntrinsicFunctionKind::AnnotationKey)
    }

    fn lower_bound_annotation_key_query(
        &mut self,
        binding: &NamedCallBinding,
        call: &ast::CallExpr,
    ) -> Result<hir::Expr, ()> {
        let [argument] = call.type_args.as_slice() else {
            self.error(
                call.span,
                "annotationKey requires exactly one explicit annotation declaration".into(),
            );
            return Err(());
        };
        if !call.args.is_empty() {
            self.error(
                call.span,
                "annotationKey does not accept value arguments".into(),
            );
            return Err(());
        }
        let ast::CallTypeArgument::Explicit(argument) = argument else {
            self.error(
                argument.span(),
                "annotationKey requires an explicit annotation declaration".into(),
            );
            return Err(());
        };
        let path = match &argument.kind {
            ast::TypeRefKind::Named(name) => std::slice::from_ref(name),
            ast::TypeRefKind::Qualified { path, arguments } if arguments.is_empty() => path,
            _ => {
                self.error(
                    argument.span,
                    "annotationKey argument must name an annotation declaration".into(),
                );
                return Err(());
            }
        };
        if !argument.annotations.is_empty() {
            self.error(
                argument.span,
                "annotationKey argument is a declaration, not an annotated type use".into(),
            );
            return Err(());
        }
        if self
            .type_params_in_scope
            .iter()
            .any(|parameter| parameter.name == path[0].text)
        {
            self.error(
                argument.span,
                "annotationKey argument must name an annotation declaration".into(),
            );
            return Err(());
        }
        let declaration = self.resolve_type_name_path(path, &[])?;
        let Some(crate::types::ResolvedTypeName::Annotation(annotation)) = declaration else {
            self.error(
                argument.span,
                "annotationKey argument must name an annotation declaration".into(),
            );
            return Err(());
        };
        let ty = match (&binding.target, &binding.origin) {
            (NamedCallTarget::Function(id), _) => self.signatures[id].return_ty,
            (NamedCallTarget::ImportedDependency(_), NamedCallOrigin::Dependency(binding)) => {
                let candidate = self
                    .dependencies
                    .as_ref()
                    .unwrap()
                    .callable_candidate(binding)
                    .expect("the selected annotation query is a callable");
                self.imported_signature_type(candidate.interface().result())
                    .map_err(|error| {
                        self.error(
                            call.span,
                            format!("cannot load annotation query result: {error:?}"),
                        );
                    })?
            }
            _ => unreachable!("annotation queries are bound core functions"),
        };
        self.prepare_type_reference_factory()
            .map_err(|error| self.error(call.span, error))?;
        Ok(hir::Expr {
            kind: hir::ExprKind::AnnotationKey(annotation),
            ty,
            span: call.span,
            origin: self.expression_origin(call.span),
        })
    }
}
