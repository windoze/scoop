use super::*;
use hir::ImportedCallableSource;

impl Lowerer {
    pub(crate) fn lower_interface_forwarder_bodies(&mut self) {
        self.lower_interface_forwarder_bodies_from(0);
    }

    pub(crate) fn lower_interface_forwarder_bodies_from(&mut self, start: usize) {
        let mut functions = self
            .interface_forwarders
            .keys()
            .copied()
            .filter(|id| id.into_raw().into_u32() as usize >= start)
            .collect::<Vec<_>>();
        functions.sort_by_key(|function| function.into_raw().into_u32());
        for function in functions {
            self.current_file = self.function_files[&function];
            let forwarder = self.interface_forwarders[&function].clone();
            let body = self.lower_synthesized_body(function, |state| {
                let span = state.functions[function].span;
                let parameters = state.functions[function].params.clone();
                let local = |parameter: &hir::Param| hir::Expr {
                    kind: hir::ExprKind::Local(parameter.local),
                    ty: parameter.ty,
                    span,
                    origin: state.expression_origin(span),
                };
                let this = local(&parameters[0]);
                let args = parameters[1..].iter().map(local).collect();
                let kind = match forwarder.receiver {
                    ForwardingReceiver::Delegate(field) => {
                        let owner = state.class_fields[field].owner;
                        let application = state.classes[owner].self_application;
                        let ty = state.classes[owner].definition.fields
                            [state.class_fields[field].definition_index]
                            .ty;
                        let receiver = hir::Expr {
                            kind: hir::ExprKind::FieldAccess {
                                receiver: Box::new(this),
                                field: state.class_field_reference(application, field),
                            },
                            ty,
                            span,
                            origin: state.expression_origin(span),
                        };
                        let Some(callee) = state.interface_delegate_callee(&forwarder.member, span)
                        else {
                            return Vec::new();
                        };
                        hir::ExprKind::MethodCall {
                            receiver: Box::new(state.adapt_to(receiver, forwarder.member.owner)),
                            callee: hir::MethodCallee::Callable(callee),
                            args,
                        }
                    }
                    ForwardingReceiver::Base { owner, callee } => {
                        hir::ExprKind::DirectSuperMethodCall {
                            receiver: Box::new(state.adapt_to(this, owner)),
                            callee: hir::MethodCallee::Callable(callee),
                            args,
                        }
                    }
                };
                vec![hir::Statement {
                    kind: hir::StatementKind::Return {
                        value: hir::Expr {
                            kind,
                            ty: state.signatures[&function].return_ty,
                            span,
                            origin: state.expression_origin(span),
                        },
                    },
                    span,
                }]
            });
            self.functions[function].kind = hir::FunctionKind::User(body);
        }
    }

    fn interface_delegate_callee(
        &mut self,
        member: &InterfaceMemberInstance,
        span: ast::Span,
    ) -> Option<hir::CallableTarget> {
        match member.member {
            hir::InterfaceMethodReference::Local(method) => {
                let Type::Interface(application) = self.types[member.owner] else {
                    unreachable!("a slot has an interface application")
                };
                let function = self.interface_method_entities[method].function;
                Some(
                    hir::Callable::Method(self.record_method_application(
                        function,
                        hir::MethodOwnerApplication::Interface(application),
                    ))
                    .into(),
                )
            }
            hir::InterfaceMethodReference::Imported { .. } => {
                let declaration = self.delegated_imported_member(member);
                self.interface_delegate_imported_callee(
                    declaration,
                    member.owner,
                    hir::MemberCallKind::Ordinary,
                    span,
                )
            }
        }
    }

    pub(super) fn interface_delegate_imported_callee(
        &mut self,
        declaration: hir::ImportedCallableDeclaration,
        owner: TypeId,
        kind: hir::MemberCallKind,
        span: ast::Span,
    ) -> Option<hir::CallableTarget> {
        let target = if matches!(
            declaration.interface().owner(),
            hir::PublicDeclarationOwnerV1::Nominal(hir::SourceNominalId::GenericTemplate(_))
        ) {
            self.resolve_imported_dispatch_callable(declaration, owner)
                .map(|target| match target {
                    hir::ImportedDispatchCallable::Template(application) => {
                        hir::CallableTarget::Application(application)
                    }
                    hir::ImportedDispatchCallable::External(callable) => {
                        hir::CallableTarget::Dependency(callable)
                    }
                })
                .map_err(|error| format!("{error:?}"))
        } else {
            self.select_imported_callable_declaration_use_with_kind(declaration, kind)
                .map(hir::CallableTarget::Dependency)
                .map_err(|error| format!("{error:?}"))
        };
        match target {
            Ok(target) => Some(target),
            Err(error) => {
                self.error(span, format!("invalid interface delegate target: {error}"));
                None
            }
        }
    }
}
