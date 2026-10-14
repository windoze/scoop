use super::*;
use crate::imports::ImportLookupLayer;
use crate::imports::lookup::calls::NamedCallBinding;

impl Lowerer {
    pub(super) fn test_call(
        &mut self,
        name: &str,
        bindings: &[NamedCallBinding],
        values: Vec<hir::Expr>,
        span: ast::Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        self.push_scope();
        let args = values
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                let name = format!("$test_argument_{index}");
                let value = self.materialize_temporary(name.clone(), value, span, sink);
                let hir::ExprKind::Local(local) = value.kind else {
                    unreachable!("a temporary is local")
                };
                self.scopes.declare(name.clone(), local);
                ast::CallArgument::positional(ast::Expr::Var(ast::Ident { text: name, span }))
            })
            .collect();
        let diagnostics_before = self.diagnostics.len();
        let result = self
            .lower_named_function_partition(
                bindings,
                ImportLookupLayer::Exact,
                &ast::CallExpr {
                    callee: ast::Ident {
                        text: name.into(),
                        span,
                    },
                    type_args: Vec::new(),
                    args,
                    span,
                },
                sink,
                None,
            )
            .ok()
            .flatten();
        self.pop_scope();
        if result.is_none() && self.diagnostics.len() == diagnostics_before {
            self.error(
                span,
                format!("test execution cannot call `{name}` with the generated arguments"),
            );
        }
        result
    }

    pub(super) fn test_api_call(
        &mut self,
        name: &str,
        values: Vec<hir::Expr>,
        span: ast::Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let binding = self.test_api_binding(name, span)?;
        self.test_call(name, &[binding], values, span, sink)
    }

    pub(super) fn test_location(
        &mut self,
        origin: hir::DefinitionOrigin,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let outer = self.current_source_context;
        self.current_source_context = Some(origin.context);
        let bindings = self
            .named_call_layers("getCurrentSourceLocation")
            .into_iter()
            .find(|layer| layer.kind == ImportLookupLayer::CorePrelude)
            .map(|layer| layer.candidates)
            .unwrap_or_default();
        let result = self.test_call(
            "getCurrentSourceLocation",
            &bindings,
            Vec::new(),
            origin.span,
            sink,
        );
        self.current_source_context = outer;
        result
    }

    pub(super) fn test_optional_action(
        &mut self,
        action: Option<hir::Expr>,
        ty: hir::TypeId,
        span: ast::Span,
    ) -> hir::Expr {
        let ty = self.option_type(ty);
        let (name, args) = match action {
            Some(action) => ("Some", vec![action]),
            None => ("None", Vec::new()),
        };
        let variant = self
            .named_enum_variant(ty, name)
            .expect("the core Option protocol is complete");
        hir::Expr {
            kind: hir::ExprKind::VariantConstruct { variant, args },
            ty,
            span,
            origin: self.expression_origin(span),
        }
    }
}
