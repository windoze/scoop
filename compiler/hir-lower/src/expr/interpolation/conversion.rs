//! Bind the core interface slot without erasing or boxing the receiver.

use super::*;
use hir::ImportedCallableSource;

impl Lowerer {
    pub(super) fn interpolation_to_string(
        &mut self,
        receiver: hir::Expr,
        span: Span,
    ) -> Option<hir::Expr> {
        if self.is_nothing_ty(receiver.ty) {
            return Some(self.adapt_to(receiver, self.string));
        }
        let interface = self.interpolation_to_string_interface(span)?;
        self.resolve_imported_member_receiver(receiver.ty, span)
            .ok()?;
        if !self.is_subtype(receiver.ty, interface) {
            self.error(
                receiver.span,
                format!(
                    "f-string expression of type `{}` does not implement core `ToString`",
                    self.type_name(receiver.ty),
                ),
            );
            return None;
        }
        let Type::Interface(bound) = self.types[interface] else {
            unreachable!("the interpolation target is the core interface");
        };
        let callee = if let Some(owner) =
            self.source_interface_id(self.interface_applications[bound].template)
        {
            let methods = self.methods_by_name(interface, "toString");
            let [method] = methods.as_slice() else {
                self.error(
                    span,
                    "core ToString requires a unique `toString` member".into(),
                );
                return None;
            };
            let member = self.interfaces[owner]
                .methods
                .iter()
                .copied()
                .find(|member| self.interface_method_entities[*member].function == method.function)
                .expect("the core interface owns its conversion member");
            self.check_call_effects(hir::Callable::Function(method.function), span);
            let application = self.record_method_application(
                method.function,
                hir::MethodOwnerApplication::Interface(bound),
            );
            self.materialize_method_callee(
                crate::CallableCandidateSource::InterfaceBound {
                    receiver_type: receiver.ty,
                    bound,
                    member,
                },
                hir::Callable::Method(application),
                &[],
            )
        } else {
            let methods = self
                .imported_member_candidates(interface, hir::ImportedMemberLookup::Name("toString"))
                .map_err(|error| self.error(span, error.to_string()))
                .ok()?;
            let [method] = methods.as_slice() else {
                self.error(
                    span,
                    "core ToString requires a unique `toString` member".into(),
                );
                return None;
            };
            let declaration = method.interface().clone();
            let declared = self
                .select_imported_callable_declaration_use_with_kind(
                    method.clone(),
                    MemberCallKind::Ordinary,
                )
                .map_err(|error| self.error(span, error.to_string()))
                .ok()?;
            self.imported_bound_member_callee(
                &declaration,
                hir::CallableTarget::Dependency(declared),
                receiver.ty,
                &[],
                self.string,
            )
            .map_err(|error| self.error(span, error))
            .ok()??
        };
        Some(hir::Expr {
            kind: ExprKind::MethodCall {
                receiver: Box::new(receiver),
                callee,
                args: Vec::new(),
            },
            ty: self.string,
            span,
            origin: self.expression_origin(span),
        })
    }

    fn interpolation_to_string_interface(&mut self, span: Span) -> Option<TypeId> {
        for binding in self.interpolation_core_bindings("ToString") {
            match binding.target {
                NamedCallTarget::Type(TopLevelTypeTarget::Nominal(
                    crate::NominalTarget::Interface(owner),
                )) => {
                    return Some(
                        self.interface_applications[self.interfaces[owner].self_application]
                            .canonical_type,
                    );
                }
                NamedCallTarget::ImportedDependency(hir::ImportedTarget::Type(owner)) => {
                    let ty = self
                        .imported_nominal_application(
                            hir::SourceNominalId::Concrete(owner.persistent()),
                            Vec::new(),
                        )
                        .map_err(|error| self.error(span, error.diagnostic("core ToString")))
                        .ok()?;
                    if matches!(self.types[ty], Type::Interface(_)) {
                        return Some(ty);
                    }
                }
                _ => {}
            }
        }
        self.error(
            span,
            "f-string requires the core ToString declaration".into(),
        );
        None
    }
}
