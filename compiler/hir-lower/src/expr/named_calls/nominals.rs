use super::*;
use crate::call_resolution::candidates::ValueParameter;
use crate::constructor_resolution::{NominalConstructorCall, ResolvedNominalConstructor};

struct NominalPlan {
    view: NominalConstructorView,
    expected: Option<TypeId>,
    fixed_alias: bool,
}

type PreparedNominalPlans = (Vec<NominalPlan>, Option<(hir::StructId, Option<TypeId>)>);

impl Lowerer {
    pub(super) fn lower_named_function_partition(
        &mut self,
        targets: &[NamedCallTarget],
        kind: ImportLookupLayer,
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Result<Option<hir::Expr>, ()> {
        let mut plans = Vec::new();
        let diagnostics_before = self.diagnostics.len();
        let mut failures = Vec::new();
        let mut intrinsics = Vec::new();
        for &target in targets {
            let mut preparation = self.clone();
            match preparation.named_nominal_plans(target, call, expected) {
                Ok((mut prepared, intrinsic)) => {
                    *self = preparation;
                    plans.append(&mut prepared);
                    if let Some(intrinsic) = intrinsic {
                        intrinsics.push(intrinsic);
                    }
                }
                Err(()) => {
                    failures.push(Box::new(preparation));
                }
            }
        }
        let explicit = self.resolve_call_type_args(&call.type_args).ok_or(())?;
        let overload = OverloadCall {
            explicit_type_args: &explicit,
            arg_exprs: &call.args,
            span: call.span,
            expected_result: expected,
            argument_protocol: CallArgumentProtocol::Ordinary,
        };
        let functions = targets
            .iter()
            .filter_map(|target| match target {
                NamedCallTarget::Function(id) => Some(*id),
                _ => None,
            })
            .collect::<Vec<_>>();
        if plans.is_empty()
            && intrinsics.is_empty()
            && !functions.is_empty()
            && functions.iter().all(|id| {
                !self.extension_receivers.contains_key(id) && !self.function_owner.contains_key(id)
            })
        {
            return match self.resolve_overload_outcome(
                &call.callee.text,
                &functions,
                &[],
                overload,
                sink,
            ) {
                crate::overload::OverloadResolutionOutcome::NoApplicable => Ok(None),
                crate::overload::OverloadResolutionOutcome::Failed => Err(()),
                crate::overload::OverloadResolutionOutcome::Resolved(resolved) => self
                    .finish_resolved_top_level_function_call(call, *resolved, sink)
                    .map(Some)
                    .ok_or(()),
            };
        }
        let mut applicable = Vec::new();
        for &target in targets {
            let NamedCallTarget::Function(function) = target else {
                continue;
            };
            let mut state = self.clone();
            let (candidate, receiver, commit) = if self.extension_receivers.contains_key(&function)
            {
                if self.current_this_ty().is_none() {
                    continue;
                }
                let Some(receiver) = state.lower_current_this(call.callee.span) else {
                    continue;
                };
                (
                    crate::CallableCandidate::function(
                        function,
                        Vec::new(),
                        state.function_lookup_witness(function),
                    ),
                    NamedCallReceiver::Extension(receiver),
                    NamedFunctionCommit::TopLevel,
                )
            } else if let Some(crate::Owner::Object(object)) =
                self.function_owner.get(&function).copied()
            {
                let Some(receiver) = state.lower_singleton_value(object, call.callee.span) else {
                    continue;
                };
                let candidate = crate::CallableCandidate {
                    function,
                    owner: crate::CallableCandidateOwner::Method(
                        state.method_owner_application(crate::Owner::Object(object), Vec::new()),
                    ),
                    source: crate::CallableCandidateSource::Direct,
                    access: crate::CallableCandidateAccess::Lookup(
                        state.function_lookup_witness(function),
                    ),
                };
                (
                    candidate,
                    NamedCallReceiver::Member(receiver),
                    NamedFunctionCommit::Member,
                )
            } else {
                (
                    crate::CallableCandidate::function(
                        function,
                        Vec::new(),
                        state.function_lookup_witness(function),
                    ),
                    NamedCallReceiver::None,
                    NamedFunctionCommit::TopLevel,
                )
            };
            match state.probe_named_callable(&call.callee.text, candidate, receiver, overload) {
                Ok(probe) => applicable.push(NamedApplicable {
                    probe: NamedFunctionLikeProbe::Callable(Box::new(probe)),
                    commit,
                }),
                Err(failure) => {
                    failures.push(failure);
                }
            }
        }
        for plan in plans {
            let arguments =
                self.named_nominal_expected_arguments(plan.view.result_type, plan.expected);
            match self.probe_named_nominal(
                plan.view,
                NominalConstructorCall {
                    explicit_type_args: &explicit,
                    expected_type_args: arguments.as_deref(),
                    arguments: &call.args,
                    span: call.span,
                },
            ) {
                Ok(mut probe) => {
                    if plan.fixed_alias {
                        probe.fix_forwarding_parameters(
                            self,
                            arguments
                                .as_deref()
                                .expect("a fixed nominal alias supplies its complete application"),
                        );
                    }
                    applicable.push(NamedApplicable {
                        probe: NamedFunctionLikeProbe::Nominal(Box::new(probe)),
                        commit: NamedFunctionCommit::Nominal,
                    });
                }
                Err(failure) => {
                    failures.push(failure);
                }
            }
        }
        for (structure, fixed) in intrinsics {
            match self.probe_expr_layer(|state, sink| {
                state.lower_ffi_struct_init(
                    structure,
                    CallSite {
                        type_args: &call.type_args,
                        args: &call.args,
                        span: call.span,
                    },
                    sink,
                    fixed,
                )
            }) {
                Ok(layer) => {
                    let ulong = self.integer_type(hir::IntegerKind::UNSIGNED_64);
                    let probe = crate::call_resolution::named::NamedIntrinsicStructProbe {
                        structure,
                        owners: self.structs[structure].type_params.clone(),
                        parameter_types: vec![ulong],
                        integer_arguments: vec![Some(hir::IntegerKind::UNSIGNED_64)],
                    };
                    applicable.push(NamedApplicable {
                        probe: NamedFunctionLikeProbe::IntrinsicStruct(probe),
                        commit: NamedFunctionCommit::Intrinsic(layer),
                    });
                }
                Err(failure) => {
                    failures.push(failure);
                }
            }
        }
        if applicable.is_empty() {
            let functions = targets
                .iter()
                .filter_map(|target| match target {
                    NamedCallTarget::Function(id)
                        if !self.extension_receivers.contains_key(id)
                            && !self.function_owner.contains_key(id) =>
                    {
                        Some(*id)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            if functions.len() > 1
                && targets
                    .iter()
                    .all(|target| matches!(target, NamedCallTarget::Function(_)))
            {
                return Ok(self.lower_top_level_function_layer(
                    &call.callee.text,
                    &functions,
                    call,
                    sink,
                    expected,
                ));
            }
            if failures.len() == 1 {
                self.commit_layer_diagnostics(*failures.pop().expect("one failed candidate"));
            } else if !failures.is_empty() {
                let mut traces = failures
                    .iter()
                    .map(|failure| {
                        failure.diagnostics[diagnostics_before..]
                            .iter()
                            .map(|diagnostic| diagnostic.message.as_str())
                            .collect::<Vec<_>>()
                            .join("; ")
                    })
                    .collect::<Vec<_>>();
                traces.sort();
                self.error(
                    call.span,
                    format!(
                        "no applicable candidate for `{}` in {} layer:\n{}",
                        call.callee.text,
                        kind.call_name(),
                        traces
                            .iter()
                            .map(|trace| format!("  - {trace}"))
                            .collect::<Vec<_>>()
                            .join("\n")
                    ),
                );
            }
            return Ok(None);
        }
        let (probes, mut commits): (Vec<_>, Vec<_>) = applicable
            .into_iter()
            .map(|applicable| (applicable.probe, applicable.commit))
            .unzip();
        let diagnostics_before = self.diagnostics.len();
        let Some(winner) = self.select_named_function_like(
            &call.callee.text,
            kind.call_name(),
            &probes,
            &call.args,
            call.span,
        ) else {
            let variants = targets
                .iter()
                .filter_map(|target| match target {
                    NamedCallTarget::Value(ValueTarget::Variant(target)) => Some(*target),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if kind == ImportLookupLayer::CorePrelude && variants.len() == targets.len() {
                self.diagnostics.truncate(diagnostics_before);
                self.ambiguous_prelude_variant(&call.callee, &variants);
            }
            return Err(());
        };
        let mut probes = probes;
        let probe = probes.swap_remove(winner);
        let commit = commits.swap_remove(winner);
        let expression = match (probe, commit) {
            (NamedFunctionLikeProbe::Callable(probe), NamedFunctionCommit::TopLevel) => {
                let resolved = self.commit_named_callable(*probe, sink);
                self.finish_resolved_top_level_function_call(call, resolved, sink)
            }
            (NamedFunctionLikeProbe::Callable(probe), NamedFunctionCommit::Member) => {
                let resolved = self.commit_named_callable(*probe, sink);
                self.finish_resolved_method_call(resolved, call.span)
            }
            (NamedFunctionLikeProbe::Nominal(probe), NamedFunctionCommit::Nominal) => {
                let resolved = self.commit_named_nominal(*probe, call.span, sink);
                Some(self.finish_named_nominal(resolved, call.span))
            }
            (NamedFunctionLikeProbe::IntrinsicStruct(_), NamedFunctionCommit::Intrinsic(layer)) => {
                Some(self.commit_expr_layer(layer, sink))
            }
            _ => unreachable!("each typed probe retains its matching commit protocol"),
        };
        expression.map(Some).ok_or(())
    }

    fn named_nominal_plans(
        &mut self,
        target: NamedCallTarget,
        call: &ast::CallExpr,
        expected: Option<TypeId>,
    ) -> Result<PreparedNominalPlans, ()> {
        let (nominal, expected, fixed_alias) = match target {
            NamedCallTarget::Value(ValueTarget::Object(id)) => {
                let ty = self.object_types[self.objects[id].object_type].canonical_type;
                if !self.type_exposes_invoke(ty, false) {
                    let kind = match self.objects[id].kind {
                        hir::ObjectKind::Standalone => "object",
                        hir::ObjectKind::Companion(_) => "companion object",
                    };
                    self.error(
                        call.span,
                        format!("{kind} `{}` cannot be constructed", call.callee.text),
                    );
                    return Err(());
                }
                return Ok((Vec::new(), None));
            }
            NamedCallTarget::Type(TopLevelTypeTarget::Nominal(nominal)) => {
                (nominal, expected, false)
            }
            NamedCallTarget::Type(TopLevelTypeTarget::Alias(id)) => {
                let ty = self
                    .resolve_type_alias_id_reference(id, &call.callee, !call.type_args.is_empty())
                    .ok_or(())?;
                let Some(nominal) = self.nominal_target_for_type(ty) else {
                    self.error(
                        call.span,
                        format!(
                            "typealias `{}` does not name a constructible type",
                            call.callee.text
                        ),
                    );
                    return Err(());
                };
                (nominal, Some(ty), true)
            }
            NamedCallTarget::Value(ValueTarget::Variant(target)) => {
                if self.resolved_variant_style(target) == VariantStyle::Unit {
                    self.error(call.span, format!("unit variant `{}` of `{}` does not take arguments; use `{}` without parentheses", call.callee.text, self.enums[target.enumeration()].name, call.callee.text));
                    return Err(());
                }
                return Ok((
                    vec![NominalPlan {
                        view: self
                            .nominal_constructor_view(NominalConstructorSource::Variant(target)),
                        expected,
                        fixed_alias: false,
                    }],
                    None,
                ));
            }
            _ => return Ok((Vec::new(), None)),
        };
        let sources = match nominal {
            crate::NominalTarget::Struct(id) => {
                if Some(id) == self.ffi_foreign_callback {
                    self.error(
                        call.span,
                        "`ForeignCallback` values can only be produced by `foreignCallback`"
                            .to_string(),
                    );
                    return Err(());
                }
                if Some(id) == self.ffi_ptr || Some(id) == self.ffi_fun_ptr {
                    return Ok((Vec::new(), Some((id, expected))));
                }
                if matches!(
                    self.structs[id].representation,
                    hir::StructRepresentation::Intrinsic(_)
                ) {
                    self.error(
                        call.span,
                        format!(
                            "intrinsic struct `{}` has no source constructor",
                            self.structs[id].name
                        ),
                    );
                    return Err(());
                }
                self.structs[id]
                    .constructors
                    .iter()
                    .copied()
                    .map(NominalConstructorSource::Struct)
                    .collect::<Vec<_>>()
            }
            crate::NominalTarget::Class(id) => {
                if let Some(kind) = self.array_class_kind(id) {
                    let mut view =
                        self.nominal_constructor_view(NominalConstructorSource::IntrinsicClass(id));
                    let element = self.intern_type(Type::Param(view.owner_parameters[0].id));
                    let opposite = match kind {
                        ArrayKind::Immutable => ArrayKind::Mutable,
                        ArrayKind::Mutable => ArrayKind::Immutable,
                    };
                    let ty = self.array_type(opposite, element);
                    view.value_parameters = vec![ValueParameter {
                        name: "source".into(),
                        calling: crate::defaults::SourceParameterCalling::Required,
                        ty,
                    }];
                    return Ok((
                        vec![NominalPlan {
                            view,
                            expected,
                            fixed_alias,
                        }],
                        None,
                    ));
                }
                if self.classes[id].modifier == hir::ClassModifier::Abstract {
                    self.error(
                        call.span,
                        format!(
                            "abstract class `{}` cannot be instantiated",
                            self.classes[id].name
                        ),
                    );
                    return Err(());
                }
                if matches!(
                    self.classes[id].representation,
                    hir::ClassRepresentation::Intrinsic(_)
                ) {
                    self.error(
                        call.span,
                        format!(
                            "intrinsic class `{}` has no source constructor",
                            self.classes[id].name
                        ),
                    );
                    return Err(());
                }
                self.classes[id]
                    .constructors
                    .iter()
                    .copied()
                    .map(NominalConstructorSource::Class)
                    .collect::<Vec<_>>()
            }
            _ => {
                self.error(
                    call.span,
                    format!(
                        "type `{}` does not name a constructible type",
                        call.callee.text
                    ),
                );
                return Err(());
            }
        };
        let mut plans = Vec::new();
        for source in sources {
            if self.constructor_is_accessible(source) {
                plans.push(NominalPlan {
                    view: self.nominal_constructor_view(source),
                    expected,
                    fixed_alias,
                });
            }
        }
        if plans.is_empty() {
            self.error(
                call.span,
                format!(
                    "constructor of type `{}` is not accessible here",
                    call.callee.text
                ),
            );
            return Err(());
        }
        Ok((plans, None))
    }

    fn named_nominal_expected_arguments(
        &self,
        definition: TypeId,
        expected: Option<TypeId>,
    ) -> Option<Vec<TypeId>> {
        match (&self.types[definition], &self.types[expected?]) {
            (Type::Class(a), Type::Class(b))
                if self.class_applications[*a].template == self.class_applications[*b].template =>
            {
                Some(self.class_applications[*b].arguments.clone())
            }
            (Type::Struct(a), Type::Struct(b))
                if self.struct_applications[*a].template
                    == self.struct_applications[*b].template =>
            {
                Some(self.struct_applications[*b].arguments.clone())
            }
            (Type::Enum(a), Type::Enum(b))
                if self.enum_applications[*a].template == self.enum_applications[*b].template =>
            {
                Some(self.enum_applications[*b].arguments.clone())
            }
            _ => None,
        }
    }

    fn finish_named_nominal(
        &mut self,
        resolved: ResolvedNominalConstructor,
        span: Span,
    ) -> hir::Expr {
        let (kind, ty) = match resolved.source {
            NominalConstructorSource::Struct(source) => {
                let owner = self.struct_constructors[source].owner;
                let application = self.struct_application_id(owner, resolved.type_args);
                let constructor = self.struct_constructor_application(source, application);
                (
                    ExprKind::StructInit {
                        constructor,
                        args: resolved.args,
                    },
                    self.struct_applications[application].canonical_type,
                )
            }
            NominalConstructorSource::Class(source) => {
                let owner = self.class_constructors[source].owner;
                let application = self.class_application_id(owner, resolved.type_args);
                let constructor = self.class_constructor_application(source, application);
                (
                    ExprKind::ClassInit {
                        constructor,
                        args: resolved.args,
                    },
                    self.class_applications[application].canonical_type,
                )
            }
            NominalConstructorSource::Variant(target) => {
                let application =
                    self.enum_application_id(target.enumeration(), resolved.type_args);
                let variant = hir::AppliedEnumVariantRef::checked(
                    &self.enums,
                    &self.enum_applications,
                    application,
                    target,
                )
                .expect("the chosen nominal application owns its variant");
                (
                    ExprKind::VariantConstruct {
                        variant,
                        args: resolved.args,
                    },
                    self.enum_applications[application].canonical_type,
                )
            }
            NominalConstructorSource::IntrinsicClass(class) => {
                let ty = self.class_application(class, resolved.type_args);
                let [argument]: [hir::Expr; 1] = resolved
                    .args
                    .try_into()
                    .expect("array conversion has one materialized argument");
                (ExprKind::ArrayClone(Box::new(argument)), ty)
            }
        };
        hir::Expr {
            kind,
            ty,
            span,
            origin: self.expression_origin(span),
        }
    }
}
