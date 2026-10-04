//! Task context uses the existing structured cleanup and managed exception paths.

use super::*;

impl BodyLowerer<'_> {
    fn context_type(&self, role: mir::ContextStorageRole) -> mir::Type {
        mir::Type::Context(mir::ContextStorageType::new(
            super::super::context::core_provider(self.module),
            role,
        ))
    }

    pub(super) fn lower_context_scope(
        &mut self,
        value: &hir::Expr,
        body: &[hir::Statement],
        span: Span,
        out: &mut Vec<smir::Statement>,
    ) {
        let key = mir::ContextKey(self.module.exact_type_identities[value.ty].id());
        let value = self.lower_expr(value);
        self.drain_prelude(span, out);
        let mark_ty = self.context_type(mir::ContextStorageRole::Mark);
        let mark = self.new_hidden("context_mark", mark_ty.clone(), false);
        out.push(smir::Statement {
            span,
            kind: smir::StatementKind::ValDecl {
                local: mark,
                init: smir::Expr::new(
                    mark_ty.clone(),
                    smir::ExprKind::Context(mir::ContextOperation::Push {
                        key,
                        value: Box::new(value),
                    }),
                ),
            },
        });
        let body = self.lower_statements(body);
        let restore = smir::Statement {
            span,
            kind: smir::StatementKind::Expr(smir::Expr::new(
                mir::Type::Unit,
                smir::ExprKind::Context(mir::ContextOperation::Restore {
                    mark: Box::new(smir::Expr::local(mark, mark_ty)),
                }),
            )),
        };
        out.push(smir::Statement {
            span,
            kind: smir::StatementKind::Try(smir::Try {
                body,
                catches: Vec::new(),
                finally_body: Some(vec![restore]),
            }),
        });
    }

    pub(super) fn lower_context_lookup(
        &mut self,
        source: &hir::Expr,
        declaration: &str,
        label: &scoop_hir::ContextParameterLabel,
        parameter: scoop_hir::ContextParameterIndex,
    ) -> smir::Expr {
        let result_ty = self.lower_type(source.ty);
        let binding_ty = self.context_type(mir::ContextStorageRole::Binding);
        let local = self.new_hidden("context_binding", binding_ty.clone(), false);
        let key = mir::ContextKey(self.module.exact_type_identities[source.ty].id());
        self.prelude.push(smir::StatementKind::ValDecl {
            local,
            init: smir::Expr::new(
                binding_ty.clone(),
                smir::ExprKind::Context(mir::ContextOperation::TryGet { key }),
            ),
        });
        let label = match label {
            scoop_hir::ContextParameterLabel::Named(name) => name.clone(),
            scoop_hir::ContextParameterLabel::Unnamed => format!("#{}", parameter.0 + 1),
        };
        let message = format!(
            "missing context {declaration}.{label}: {}",
            mir::type_name(self.shell, &result_ty)
        );
        let failure = self.missing_context_exception(message, source.span);
        self.prelude.push(smir::StatementKind::If {
            cond: smir::Expr::new(
                mir::Type::Boolean,
                smir::ExprKind::Context(mir::ContextOperation::IsPresent {
                    binding: Box::new(smir::Expr::local(local, binding_ty.clone())),
                }),
            ),
            then_body: Vec::new(),
            else_body: Some(vec![failure]),
        });
        smir::Expr::new(
            result_ty,
            smir::ExprKind::Context(mir::ContextOperation::UnwrapBinding {
                binding: Box::new(smir::Expr::local(local, binding_ty)),
            }),
        )
    }

    fn missing_context_exception(&mut self, message: String, span: Span) -> smir::Statement {
        let (class, initializer) = match self.core_protocols {
            hir::ConcreteCoreProtocols::Defined(protocols) => {
                let constructor = protocols.exceptions.missing_context_constructor;
                (
                    self.class_map[&self.module.class_constructors[constructor].class],
                    mir::Callee::User(self.ctors[&constructor]),
                )
            }
            hir::ConcreteCoreProtocols::Imported(protocols) => self.imported_exception_constructor(
                protocols
                    .exceptions()
                    .missing_context_exception()
                    .persistent(),
                protocols
                    .exceptions()
                    .missing_context_exception_constructor(),
            ),
        };
        let message_ty = self
            .module
            .types
            .iter()
            .find_map(|(ty, value)| {
                let hir::TypeKind::Enum(enumeration) = value.kind else {
                    return None;
                };
                let option = self.module.option_core(enumeration)?;
                let payload = option.some_payload();
                let field = &self.module.enums[enumeration].variants
                    [payload.variant().variant().into_raw() as usize]
                    .fields[payload.local_index() as usize];
                matches!(self.module.types[field.ty].kind, hir::TypeKind::String).then_some(ty)
            })
            .expect("the actual MissingContextException constructor materializes Option<String>");
        let message_ty = self.lower_type(message_ty);
        let some = option_core_for_type(self.module, self.enums, &message_ty).some();
        let message = self.intern_current_string(message);
        let argument = smir::Expr::new(
            message_ty,
            smir::ExprKind::VariantConstruct {
                variant: some,
                fields: vec![smir::Expr::new(
                    mir::Type::String,
                    smir::ExprKind::StringConst(message),
                )],
            },
        );
        smir::Statement {
            span,
            kind: smir::StatementKind::Throw(smir::Expr::new(
                mir::Type::Class(class),
                smir::ExprKind::ClassNew {
                    class_id: class,
                    publish_release: false,
                    initializer,
                    args: vec![argument],
                },
            )),
        }
    }
}
