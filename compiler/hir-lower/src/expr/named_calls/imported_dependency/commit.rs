use super::candidate::{ImportedCallableCandidate, NormalizedImportedIntrinsic};
use hir::ImportedCallableSource;
use scoop_hir as hir;

use super::{ImportedCallReceiver, ImportedDependencyCallProbe, ImportedMemberReceiver};
use crate::Lowerer;
use crate::argument_materialization::{ArgumentEvaluation, ResolvedArgumentMaterialization};
use crate::expr::MemberCallKind;

impl Lowerer {
    pub(crate) fn commit_imported_dependency_callable(
        &mut self,
        probe: ImportedDependencyCallProbe,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        self.commit_imported_dependency_callable_with_kind(probe, sink, MemberCallKind::Ordinary)
    }

    pub(in crate::expr) fn commit_imported_dependency_callable_with_kind(
        &mut self,
        probe: ImportedDependencyCallProbe,
        sink: &mut Vec<hir::Statement>,
        kind: MemberCallKind,
    ) -> Option<hir::Expr> {
        self.commit_imported_callable_arguments(probe, sink, kind, ArgumentEvaluation::Source)
    }

    pub(in crate::expr) fn commit_imported_lowered_callable(
        &mut self,
        probe: ImportedDependencyCallProbe,
    ) -> Option<hir::Expr> {
        let mut sink = Vec::new();
        let expression = self.commit_imported_callable_arguments(
            probe,
            &mut sink,
            MemberCallKind::Ordinary,
            ArgumentEvaluation::Lowered,
        );
        assert!(
            sink.is_empty(),
            "prepared calls have already lowered, required arguments"
        );
        expression
    }

    fn commit_imported_callable_arguments(
        &mut self,
        probe: ImportedDependencyCallProbe,
        sink: &mut Vec<hir::Statement>,
        kind: MemberCallKind,
        evaluation: ArgumentEvaluation,
    ) -> Option<hir::Expr> {
        let ImportedDependencyCallProbe {
            implementation,
            state,
            candidate,
            receiver,
            source_args,
            argument_sinks,
            argument_map,
            default_plan,
            parameter_types,
            result_type,
            call_span,
            ..
        } = probe;
        *self = *state;
        if kind == MemberCallKind::DirectSuper
            && candidate.interface().modality() == hir::CallableModalityV1::Abstract
        {
            self.error(
                call_span,
                "abstract dependency member cannot be called with `super`".into(),
            );
            return None;
        }
        if candidate.interface().effects().execution() == scoop_identity::Effect::Suspend {
            let declaration = self
                .dependencies
                .as_ref()
                .expect("a dependency call has its declaration catalog")
                .callable_declaration(candidate.interface().declaration())
                .expect("a selected call retains its original declaration");
            self.check_suspend_context(declaration.name(), call_span);
        }
        if candidate.interface().effects().safety() == hir::CallableSafetyV1::Unsafe {
            self.require_unsafe_operation(
                call_span,
                &format!("calling an unsafe dependency {}", candidate.description()),
            );
        }

        if let super::ImportedCallImplementation::Intrinsic {
            operation: super::ImportedIntrinsicCall::Expression(expression),
            ..
        } = &implementation
        {
            return Some(expression.clone());
        }

        let (receiver, source_receiver) = match receiver {
            ImportedCallReceiver::Absent => (None, hir::SourceCallReceiver::NoReceiver),
            ImportedCallReceiver::Member {
                value: ImportedMemberReceiver::Value(receiver),
                static_type,
            } => (
                Some(receiver),
                hir::SourceCallReceiver::Receiver { static_type },
            ),
            ImportedCallReceiver::Member {
                value: ImportedMemberReceiver::LiteralSubject(_),
                ..
            } => {
                unreachable!("literal subjects commit through the pattern equality entry")
            }
        };
        let bound_declaration = receiver
            .as_ref()
            .filter(|receiver| matches!(self.types[receiver.ty], hir::Type::Param(_)))
            .map(|_| candidate.interface().clone());
        let parameters = candidate
            .source_interface()
            .expect("callable candidates retain their validated source interface")
            .parameters()
            .parameters()
            .iter()
            .zip(parameter_types.iter().copied())
            .map(|(parameter, ty)| (parameter.name().as_str().to_owned(), ty))
            .collect::<Vec<_>>();
        let (receiver, parameter_values) = self.materialize_argument_inputs(
            ResolvedArgumentMaterialization {
                parameters: &parameters,
                inputs: argument_map.parameters(),
                receiver,
                source_args,
                argument_sinks,
                call_span,
                evaluation,
                temporary_prefix: "$dependency.",
            },
            sink,
            |state, template, context| {
                let Some(prepared) = default_plan.get(*template) else {
                    state.error(
                        call_span,
                        format!("dependency default plan is missing template {template:?}"),
                    );
                    return None;
                };
                match state.materialize_imported_default(
                    prepared,
                    context.receiver,
                    context.parameters,
                    context.call_span,
                    context.sink,
                ) {
                    Ok(value) => Some(value),
                    Err(error) => {
                        state.error(
                            call_span,
                            format!("failed to materialize dependency default: {error}"),
                        );
                        None
                    }
                }
            },
        )?;

        if let scoop_identity::CallableTemplateOrigin::VariantConstructor(variant) =
            candidate.interface().declaration()
        {
            return Some(hir::Expr {
                kind: hir::ExprKind::VariantConstruct {
                    variant: hir::EnumVariantApplication {
                        owner: result_type,
                        variant,
                    },
                    args: parameter_values,
                },
                ty: result_type,
                span: call_span,
                origin: self.expression_origin(call_span),
            });
        }

        if let Some(intrinsic) = candidate.normalized_intrinsic() {
            let receiver = receiver.expect("a resolved imported intrinsic member has a receiver");
            let kind = match intrinsic {
                NormalizedImportedIntrinsic::Integer(kind) => {
                    return Some(self.normalize_integer_method_call(
                        kind,
                        receiver,
                        &parameter_values,
                        result_type,
                        call_span,
                    ));
                }
                NormalizedImportedIntrinsic::Unary(kind) => {
                    assert!(
                        parameter_values.is_empty(),
                        "a validated unary intrinsic has no value parameters"
                    );
                    hir::ExprKind::PrimitiveUnary {
                        kind,
                        operand: Box::new(receiver),
                    }
                }
                NormalizedImportedIntrinsic::Binary(kind) => {
                    let [argument] = parameter_values.as_slice() else {
                        unreachable!("a validated binary intrinsic has one value parameter")
                    };
                    hir::ExprKind::PrimitiveBinary {
                        kind,
                        lhs: Box::new(receiver),
                        rhs: Box::new(argument.clone()),
                    }
                }
            };
            return Some(hir::Expr {
                kind,
                ty: result_type,
                span: call_span,
                origin: self.expression_origin(call_span),
            });
        }
        if let super::ImportedCallImplementation::Intrinsic { operation, .. } = implementation {
            let receiver = receiver.expect("a resolved intrinsic member has a receiver");
            return Some(match operation {
                super::ImportedIntrinsicCall::PointerMember(intrinsic) => self
                    .normalize_pointer_intrinsic(
                        intrinsic,
                        receiver,
                        parameter_values,
                        result_type,
                        call_span,
                    ),
                super::ImportedIntrinsicCall::ArrayConversion(intrinsic) => self
                    .normalize_array_intrinsic_call(
                        hir::IntrinsicFunctionKind::Array(intrinsic),
                        receiver,
                        &parameter_values,
                        result_type,
                        call_span,
                    )?,
                super::ImportedIntrinsicCall::ArrayAccess(intrinsic) => self
                    .normalize_array_intrinsic_call(
                        hir::IntrinsicFunctionKind::ArrayAccess(intrinsic),
                        receiver,
                        &parameter_values,
                        result_type,
                        call_span,
                    )?,
                super::ImportedIntrinsicCall::Expression(_) => {
                    unreachable!(
                        "expression intrinsics were committed before argument materialization"
                    )
                }
            });
        }
        let mut args = Vec::with_capacity(parameter_values.len() + usize::from(receiver.is_some()));
        args.extend(receiver);
        args.extend(parameter_values);

        if let super::ImportedCallImplementation::Generic {
            template,
            arguments,
        } = implementation
        {
            let binding = match candidate {
                ImportedCallableCandidate::Binding(candidate) => {
                    Some(std::sync::Arc::new(candidate.binding().clone()))
                }
                ImportedCallableCandidate::Declaration(_) => None,
            };
            let kind = match template {
                super::generic::ImportedGenericTarget::Function(template) => {
                    let application = self.imported_generic_applications.alloc(
                        hir::ImportedGenericCallableApplication {
                            template,
                            arguments,
                        },
                    );
                    let bound = match self.imported_bound_call_kind(
                        bound_declaration.as_ref(),
                        hir::CallableTarget::Application(application),
                        &mut args,
                        &parameter_types,
                        result_type,
                    ) {
                        Ok(bound) => bound,
                        Err(message) => {
                            self.error(call_span, message);
                            return None;
                        }
                    };
                    bound.unwrap_or_else(|| {
                        self.resolved_template_call(
                            application,
                            kind,
                            binding,
                            args,
                            source_receiver,
                        )
                    })
                }
                super::generic::ImportedGenericTarget::Constructor(template) => {
                    match self.constructor_template_application(template, result_type) {
                        hir::ConstructorApplicationRef::Class(constructor) => {
                            hir::ExprKind::ClassInit { constructor, args }
                        }
                        hir::ConstructorApplicationRef::Struct(constructor) => {
                            hir::ExprKind::StructInit { constructor, args }
                        }
                    }
                }
                super::generic::ImportedGenericTarget::Variant(_) => {
                    unreachable!("enum variants were constructed before callable dispatch")
                }
                super::generic::ImportedGenericTarget::Intrinsic(_) => {
                    unreachable!("intrinsic operations were normalized before callable dispatch")
                }
            };
            return Some(hir::Expr {
                kind,
                ty: result_type,
                span: call_span,
                origin: self.expression_origin(call_span),
            });
        }
        let selected = match candidate {
            ImportedCallableCandidate::Binding(candidate) => self
                .select_imported_dependency_callable_use(*candidate)
                .map(|(callee, binding)| (callee, Some(binding))),
            ImportedCallableCandidate::Declaration(candidate) => self
                .select_imported_callable_declaration_use_with_kind(*candidate, kind)
                .map(|callee| (callee, None)),
        };
        let (callee, binding) = match selected {
            Ok(selected) => selected,
            Err(error) => {
                self.error(
                    call_span,
                    format!("failed to select imported dependency callable: {error}"),
                );
                return None;
            }
        };
        let bound = match self.imported_bound_call_kind(
            bound_declaration.as_ref(),
            hir::CallableTarget::Dependency(callee),
            &mut args,
            &parameter_types,
            result_type,
        ) {
            Ok(bound) => bound,
            Err(message) => {
                self.error(call_span, message);
                return None;
            }
        };
        Some(hir::Expr {
            kind: bound.unwrap_or(hir::ExprKind::Call {
                callee: hir::CallableTarget::Dependency(callee),
                binding,
                args,
                receiver: source_receiver,
            }),
            ty: result_type,
            span: call_span,
            origin: self.expression_origin(call_span),
        })
    }
}
