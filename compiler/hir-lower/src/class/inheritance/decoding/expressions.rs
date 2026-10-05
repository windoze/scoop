use super::*;
use crate::expr::{CallSite, RequiredCallableModifiers};

impl Lowerer {
    pub(super) fn decoding_expr(&self, kind: hir::ExprKind, ty: TypeId, span: Span) -> hir::Expr {
        hir::Expr {
            kind,
            ty,
            span,
            origin: self.expression_origin(span),
        }
    }

    pub(super) fn decoding_string(&mut self, value: &str, span: Span) -> hir::Expr {
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

    pub(super) fn decoding_long(&mut self, value: u64, span: Span) -> hir::Expr {
        let ty = self.intern_type(Type::Integer(hir::IntegerKind::SIGNED_64));
        self.decoding_expr(
            hir::ExprKind::IntegerLiteral(hir::HirIntegerConstant::Signed64(value)),
            ty,
            span,
        )
    }

    pub(super) fn decoding_local(
        &mut self,
        value: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> hir::Expr {
        self.materialize_temporary("__decoded_value".into(), value, span, sink)
    }

    pub(super) fn decoding_arguments(
        &mut self,
        values: Vec<hir::Expr>,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Vec<ast::CallArgument> {
        values
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                let name = format!("__decoded_argument_{index}");
                let value = self.materialize_temporary(name.clone(), value, span, sink);
                let hir::ExprKind::Local(local) = value.kind else {
                    unreachable!("a materialized argument is a local")
                };
                self.scopes.declare(name.clone(), local);
                ast::CallArgument::positional(ast::Expr::Var(ast::Ident { text: name, span }))
            })
            .collect()
    }

    pub(super) fn decoding_call(
        &mut self,
        receiver: hir::Expr,
        name: &str,
        values: Vec<hir::Expr>,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let receiver = self.decoding_local(receiver, span, sink);
        self.push_scope();
        let arguments = self.decoding_arguments(values, span, sink);
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

    pub(super) fn decoding_property(
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

    pub(super) fn end_decoding_container(
        &mut self,
        receiver: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<()> {
        let value = self.decoding_call(receiver, "end", Vec::new(), span, sink)?;
        sink.push(hir::Statement {
            kind: hir::StatementKind::Expr(value),
            span,
        });
        Some(())
    }

    pub(super) fn decoding_branch(
        &mut self,
        condition: hir::Expr,
        ty: TypeId,
        mut yes: (Vec<hir::Statement>, hir::Expr),
        mut no: (Vec<hir::Statement>, hir::Expr),
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> hir::Expr {
        let local = self.alloc_hidden("decoded_branch", ty);
        yes.0.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local },
                init: yes.1,
            },
            span,
        });
        no.0.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local },
                init: no.1,
            },
            span,
        });
        sink.push(hir::Statement {
            kind: hir::StatementKind::If {
                cond: condition,
                then_body: yes.0,
                else_body: Some(no.0),
            },
            span,
        });
        self.decoding_expr(hir::ExprKind::Local(local), ty, span)
    }

    pub(super) fn decoding_failure(
        &mut self,
        decoder: hir::Expr,
        message: &str,
        span: Span,
    ) -> Option<Vec<hir::Statement>> {
        let exception = self.core_coding_nominal("DecodingException")?;
        let ty = self
            .apply_nominal_type(exception, Vec::new())
            .expect("the core exception is a nominal type");
        let record = self.decoding_primary(ty, span)?;
        let path = self.decoding_property(decoder, "path", span)?;
        let message = self.decoding_string(message, span);
        let mut statements = Vec::new();
        let value = self.call_decoding_constructor(
            ty,
            &record,
            vec![path, message],
            span,
            &mut statements,
        )?;
        statements.push(hir::Statement {
            kind: hir::StatementKind::Throw(value),
            span,
        });
        Some(statements)
    }
}
