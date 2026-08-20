//! F-strings bind the actual core declarations and emit ordinary typed calls.

use super::*;
use crate::imports::ImportLookupLayer;
use crate::imports::lookup::calls::{NamedCallBinding, NamedCallOrigin, NamedCallTarget};
use crate::namespace::{TopLevelNamespaces, TopLevelTypeTarget};
use members::PropertyExtensionInvokeOutcome;

impl Lowerer {
    pub(super) fn lower_interpolated_string(
        &mut self,
        parts: &[ast::StringPart],
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let builder = self.construct_interpolation_builder(span, sink)?;
        let ty = builder.ty;
        let local = self.alloc_hidden("stringBuilder", ty);
        sink.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local },
                init: builder,
            },
            span,
        });
        let receiver = hir::Expr {
            kind: ExprKind::Local(local),
            ty,
            span,
            origin: self.expression_origin(span),
        };
        for part in parts {
            let (value, part_span) = match part {
                ast::StringPart::Text { value, span } => (
                    ast::Expr::StringLiteral {
                        value: value.clone(),
                        span: *span,
                    },
                    *span,
                ),
                ast::StringPart::Expression { value, span } => (value.as_ref().clone(), *span),
            };
            let addition = self.interpolation_member_call(
                receiver.clone(),
                "add",
                &[ast::CallArgument::positional(value)],
                part_span,
                sink,
            )?;
            sink.push(hir::Statement {
                kind: hir::StatementKind::Expr(addition),
                span: part_span,
            });
        }
        self.interpolation_member_call(receiver, "build", &[], span, sink)
    }

    fn construct_interpolation_builder(
        &mut self,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        // This is the core prelude's typed binding, independent of every
        // user scope, import, alias, or same-name function candidate.
        let bindings = if self.current_cone() == scoop_identity::ConeIdentity::CORE {
            self.top_level_namespaces
                .current_type_bindings()
                .into_iter()
                .filter(|(package, name, _, _)| {
                    *package == TopLevelNamespaces::root_package() && name == "StringBuilder"
                })
                .map(|(_, _, target, _)| {
                    let target = NamedCallTarget::Type(target);
                    NamedCallBinding {
                        target,
                        origin: NamedCallOrigin::Core(target),
                    }
                })
                .collect::<Vec<_>>()
        } else {
            self.named_call_layers("StringBuilder")
                .into_iter()
                .filter(|layer| layer.kind == ImportLookupLayer::CorePrelude)
                .flat_map(|layer| layer.candidates)
                .collect()
        };
        let targets = bindings
            .into_iter()
            .filter(|binding| {
                matches!(
                    binding.target,
                    NamedCallTarget::Type(TopLevelTypeTarget::Nominal(
                        crate::NominalTarget::Class(_)
                    )) | NamedCallTarget::ImportedDependency(hir::ImportedTarget::Type(_))
                )
            })
            .collect::<Vec<_>>();
        if targets.is_empty() {
            self.error(
                span,
                "f-string requires the core StringBuilder declaration".into(),
            );
            return None;
        }
        let call = ast::CallExpr {
            callee: ast::Ident {
                text: "StringBuilder".into(),
                span,
            },
            type_args: Vec::new(),
            args: Vec::new(),
            span,
        };
        self.lower_named_function_partition(
            &targets,
            ImportLookupLayer::CorePrelude,
            &call,
            sink,
            None,
        )
        .ok()
        .flatten()
    }

    fn interpolation_member_call(
        &mut self,
        receiver: hir::Expr,
        member: &str,
        args: &[ast::CallArgument],
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let name = ast::Ident {
            text: member.into(),
            span,
        };
        let candidates = self.methods_by_name(receiver.ty, member);
        match self.probe_member_call_partition(
            candidates,
            &name,
            receiver,
            CallSite {
                type_args: &[],
                args,
                span,
            },
            None,
            RequiredCallableModifiers::default(),
        ) {
            PropertyExtensionInvokeOutcome::Resolved(layer) => {
                Some(self.commit_expr_layer(layer, sink))
            }
            PropertyExtensionInvokeOutcome::Failed(failure)
            | PropertyExtensionInvokeOutcome::NoApplicable(Some(failure)) => {
                self.commit_layer_diagnostics(*failure);
                None
            }
            PropertyExtensionInvokeOutcome::Blocked => None,
            PropertyExtensionInvokeOutcome::NoApplicable(None) => {
                self.error(
                    span,
                    format!("core StringBuilder has no applicable `{member}` member"),
                );
                None
            }
        }
    }
}
