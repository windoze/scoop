use super::*;
use crate::expr::{CallSite, RequiredCallableModifiers};

impl Lowerer {
    pub(in crate::class::inheritance) fn coding_expr(
        &self,
        kind: hir::ExprKind,
        ty: TypeId,
        span: Span,
    ) -> hir::Expr {
        hir::Expr {
            kind,
            ty,
            span,
            origin: self.expression_origin(span),
        }
    }

    pub(in crate::class::inheritance) fn coding_string(
        &mut self,
        value: &str,
        span: Span,
    ) -> hir::Expr {
        self.lower_expr(
            &ast::Expr::StringLiteral {
                value: value.into(),
                span,
            },
            &mut Vec::new(),
            Some(self.string),
        )
        .expect("a String constant is valid in a generated method")
    }

    pub(in crate::class::inheritance) fn coding_local(
        &mut self,
        value: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> hir::Expr {
        self.materialize_temporary("__coding_value".into(), value, span, sink)
    }

    pub(in crate::class::inheritance) fn coding_arguments(
        &mut self,
        values: Vec<hir::Expr>,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Vec<ast::CallArgument> {
        values
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                let name = format!("__coding_argument_{index}");
                let value = self.materialize_temporary(name.clone(), value, span, sink);
                let hir::ExprKind::Local(local) = value.kind else {
                    unreachable!("a materialized argument is a local")
                };
                self.scopes.declare(name.clone(), local);
                ast::CallArgument::positional(ast::Expr::Var(ast::Ident { text: name, span }))
            })
            .collect()
    }

    pub(in crate::class::inheritance) fn coding_call(
        &mut self,
        receiver: hir::Expr,
        name: &str,
        values: Vec<hir::Expr>,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let receiver = self.coding_local(receiver, span, sink);
        self.push_scope();
        let arguments = self.coding_arguments(values, span, sink);
        let result = self.lower_explicit_named_call(
            receiver,
            &ast::Ident {
                text: name.into(),
                span,
            },
            CallSite {
                type_args: &[],
                args: &arguments,
                span,
            },
            sink,
            None,
            RequiredCallableModifiers::default(),
        );
        self.pop_scope();
        result
    }

    pub(in crate::class::inheritance) fn coding_property(
        &mut self,
        receiver: hir::Expr,
        name: &str,
        span: Span,
    ) -> Option<hir::Expr> {
        self.member_property_read(
            receiver,
            &ast::Ident {
                text: name.into(),
                span,
            },
        )
    }

    pub(in crate::class::inheritance) fn end_coding_container(
        &mut self,
        receiver: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<()> {
        let value = self.coding_call(receiver, "end", Vec::new(), span, sink)?;
        sink.push(hir::Statement {
            kind: hir::StatementKind::Expr(value),
            span,
        });
        Some(())
    }
}
