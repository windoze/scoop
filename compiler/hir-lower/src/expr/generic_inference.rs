//! Generic argument inference and structural type-parameter binding.

use super::*;

use crate::call_resolution::applicability::NominalApplicabilityInput;
use crate::call_resolution::arguments::SourceInputKind;
use crate::call_resolution::candidates::{NominalConstructorSource, NominalConstructorView};
use crate::call_resolution::constraints::{
    ConstraintFailure, ConstraintFailureKind, ConstraintOrigin,
};
use crate::call_resolution::diagnostics::{
    nominal_source_signature, render_nominal_constraint_failure,
};

impl Lowerer {
    pub(super) fn lower_nominal_arguments(
        &mut self,
        input: NominalArgumentInput<'_>,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<NominalArguments> {
        let NominalArgumentInput {
            view,
            argument_map,
            expressions: arg_exprs,
            explicit_type_args: explicit_arguments,
            expected_type_args: expected_arguments,
            span,
        } = input;
        debug_assert_eq!(argument_map.source_order.len(), arg_exprs.len());
        let mut seed = vec![None; view.owner_parameters.len()];
        if !explicit_arguments.is_empty() {
            for (binding, &argument) in seed.iter_mut().zip(explicit_arguments) {
                *binding = Some(argument);
            }
        } else if let Some(expected_arguments) = expected_arguments {
            for (binding, &argument) in seed.iter_mut().zip(expected_arguments) {
                *binding = Some(argument);
            }
        }

        let mut lowered = vec![None; arg_exprs.len()];
        let mut sinks: Vec<Vec<hir::Statement>> =
            (0..arg_exprs.len()).map(|_| Vec::new()).collect();
        for input in &argument_map.source_order {
            let source_index = input.index();
            let (parameter, kind) = argument_map.source_binding(*input);
            let parameter = &view.value_parameters[parameter.index()];
            let parameter = match (&parameter.calling, kind) {
                (
                    crate::defaults::SourceParameterCalling::Vararg {
                        element_type: element_ty,
                        ..
                    },
                    SourceInputKind::VarargElement,
                ) => *element_ty,
                (
                    crate::defaults::SourceParameterCalling::Vararg { .. },
                    SourceInputKind::VarargArray,
                )
                | (
                    crate::defaults::SourceParameterCalling::Required
                    | crate::defaults::SourceParameterCalling::Default(_),
                    SourceInputKind::Value,
                ) => parameter.ty,
                _ => unreachable!("argument mapping fixes each input shape"),
            };
            let hint = self.try_substitute(parameter, &seed);
            if hint.is_none()
                && self.expr_requires_expected_type(&arg_exprs[source_index].expression)
            {
                continue;
            }
            lowered[source_index] = Some(self.lower_expr(
                &arg_exprs[source_index].expression,
                &mut sinks[source_index],
                hint,
            )?);
        }

        let mut type_args = loop {
            let argument_types = lowered
                .iter()
                .map(|argument| argument.as_ref().map(|argument| argument.ty))
                .collect::<Vec<_>>();
            match self.solve_nominal_applicability(NominalApplicabilityInput {
                view,
                argument_map,
                explicit_arguments,
                expected_arguments,
                argument_types: &argument_types,
            }) {
                Ok(arguments) => break arguments,
                Err(failure)
                    if matches!(failure.kind, ConstraintFailureKind::NoUniqueSolution { .. })
                        && lowered.iter().any(Option::is_none) =>
                {
                    // No other relation can type the first postponed input.
                    // Lower it without a hint to preserve its focused source
                    // diagnostic (`None`, `[]`, or an untyped callable).
                    let source_index = lowered
                        .iter()
                        .position(Option::is_none)
                        .expect("a postponed nominal argument remains");
                    lowered[source_index] = Some(self.lower_expr(
                        &arg_exprs[source_index].expression,
                        &mut sinks[source_index],
                        None,
                    )?);
                }
                Err(failure) => {
                    self.diagnose_nominal_failure(view, argument_map, &lowered, failure, span);
                    return None;
                }
            }
        };

        for input in &argument_map.source_order {
            let source_index = input.index();
            if lowered[source_index].is_some() {
                continue;
            }
            let (parameter, kind) = argument_map.source_binding(*input);
            let parameter = &view.value_parameters[parameter.index()];
            let parameter = match (&parameter.calling, kind) {
                (
                    crate::defaults::SourceParameterCalling::Vararg {
                        element_type: element_ty,
                        ..
                    },
                    SourceInputKind::VarargElement,
                ) => *element_ty,
                (
                    crate::defaults::SourceParameterCalling::Vararg { .. },
                    SourceInputKind::VarargArray,
                )
                | (
                    crate::defaults::SourceParameterCalling::Required
                    | crate::defaults::SourceParameterCalling::Default(_),
                    SourceInputKind::Value,
                ) => parameter.ty,
                _ => unreachable!("argument mapping fixes each input shape"),
            };
            let expected = self.instantiate_ty(parameter, &type_args);
            lowered[source_index] = Some(self.lower_expr(
                &arg_exprs[source_index].expression,
                &mut sinks[source_index],
                Some(expected),
            )?);
        }

        let argument_types = lowered
            .iter()
            .map(|argument| argument.as_ref().map(|argument| argument.ty))
            .collect::<Vec<_>>();
        type_args = match self.solve_nominal_applicability(NominalApplicabilityInput {
            view,
            argument_map,
            explicit_arguments,
            expected_arguments,
            argument_types: &argument_types,
        }) {
            Ok(arguments) => arguments,
            Err(failure) => {
                self.diagnose_nominal_failure(view, argument_map, &lowered, failure, span);
                return None;
            }
        };

        let mut args = Vec::with_capacity(lowered.len());
        for (argument, mut argument_sink) in lowered.into_iter().zip(sinks) {
            sink.append(&mut argument_sink);
            args.push(argument.expect("the solved nominal types every postponed argument"));
        }
        Some(NominalArguments { args, type_args })
    }

    pub(super) fn diagnose_nominal_failure(
        &mut self,
        view: &NominalConstructorView,
        argument_map: &crate::call_resolution::arguments::CandidateArgumentMap,
        arguments: &[Option<hir::Expr>],
        failure: ConstraintFailure,
        span: Span,
    ) {
        let diagnostic_span = match failure.origin {
            ConstraintOrigin::Argument(input) => arguments
                .get(input.index())
                .and_then(Option::as_ref)
                .map_or(span, |argument| argument.span),
            _ => span,
        };
        let reason =
            render_nominal_constraint_failure(self, view, argument_map, arguments, &failure);
        self.nominal_candidate_diagnostic(view, diagnostic_span, &reason);
    }

    pub(super) fn diagnose_nominal_shape_failure(
        &mut self,
        view: &NominalConstructorView,
        span: Span,
        reason: String,
    ) {
        self.nominal_candidate_diagnostic(view, span, &reason);
    }

    fn nominal_candidate_diagnostic(
        &mut self,
        view: &NominalConstructorView,
        span: Span,
        reason: &str,
    ) {
        let target = self.nominal_value_name(view.target);
        let signature = nominal_source_signature(self, view);
        self.error(
            span,
            format!(
                "no applicable candidate for constructor `{target}` in nominal constructor candidate layer:\n  - {signature} — {reason}"
            ),
        );
    }

    fn nominal_value_name(&self, target: NominalConstructorSource) -> String {
        match target {
            NominalConstructorSource::Struct(structure) => self.structs[structure].name.clone(),
            NominalConstructorSource::Class(class) => self.classes[class].name.clone(),
            NominalConstructorSource::Variant {
                enumeration,
                variant,
            } => format!(
                "{}.{}",
                self.enums[enumeration].name,
                self.enums[enumeration].variants[variant as usize].name,
            ),
        }
    }

    /// Expressions whose type cannot be synthesized without context. Calls
    /// to generic constructors are contextual only when their own
    /// context-independent arguments cannot bind every constructor variable;
    /// this lets nested calls perform their own fixed-point inference.
    pub(crate) fn expr_requires_expected_type(&self, expr: &ast::Expr) -> bool {
        match expr {
            ast::Expr::Var(name) => {
                name.text == "None"
                    && self.scopes.lookup(&name.text).is_none()
                    && !self.host_has_property(&name.text)
            }
            ast::Expr::FieldAccess(access) => self.unit_variant_from_field(access).is_some(),
            ast::Expr::TupleLiteral { elements, .. } => {
                elements.is_empty()
                    || elements
                        .iter()
                        .any(|element| self.expr_requires_expected_type(element))
            }
            // The surrounding type also chooses between Array and
            // MutableArray, even when every element is independently typed.
            ast::Expr::ArrayLiteral { .. } => true,
            ast::Expr::Call(call) if !call.type_args.is_empty() => false,
            ast::Expr::Call(call) => self.constructor_requires_expected(&call.callee, &call.args),
            ast::Expr::StructInit { name, args, .. } => {
                self.constructor_requires_expected(name, args)
            }
            ast::Expr::MethodCall {
                receiver,
                name,
                type_args,
                args,
                ..
            } => {
                type_args.is_empty()
                    && self.qualified_variant_requires_expected(receiver, name, args)
            }
            // A lambda with an explicit, fully typed parameter header can
            // synthesize its own function type and therefore participate in
            // generic inference before an overload is selected. Untyped or
            // omitted parameters remain contextual and are probed against
            // each candidate transactionally.
            ast::Expr::Lambda {
                parameters: Some(parameters),
                ..
            } if parameters.iter().all(|parameter| parameter.ty.is_some()) => false,
            ast::Expr::Lambda { .. }
            | ast::Expr::AnonymousFunction { .. }
            | ast::Expr::CallableReference { .. } => true,
            // Structured expressions perform their own branch-level fixed
            // point and therefore do not need to be postponed as a whole.
            ast::Expr::If(_) | ast::Expr::When(_) | ast::Expr::Try(_) => false,
            _ => false,
        }
    }

    pub(super) fn constructor_requires_expected(
        &self,
        name: &ast::Ident,
        args: &[ast::CallArgument],
    ) -> bool {
        if let Some(&(class, _)) = self.classes_by_name.get(&name.text)
            && self.array_class_kind(class).is_some()
        {
            let [source] = args else {
                return false;
            };
            return self.expr_requires_expected_type(&source.expression);
        }
        let Some((type_param_count, fields)) = self.constructor_inference_shape(&name.text) else {
            return false;
        };
        if type_param_count == 0 {
            return false;
        }
        let mut bound = vec![false; type_param_count];
        for (arg, field) in args.iter().zip(fields) {
            if !self.expr_requires_expected_type(&arg.expression) {
                self.mark_type_params(field, &mut bound);
            }
        }
        bound.iter().any(|bound| !bound)
    }

    pub(super) fn qualified_variant_requires_expected(
        &self,
        receiver: &ast::Expr,
        name: &ast::Ident,
        args: &[ast::CallArgument],
    ) -> bool {
        let ast::Expr::Var(enum_name) = receiver else {
            return false;
        };
        if self.scopes.lookup(&enum_name.text).is_some() || self.host_has_property(&enum_name.text)
        {
            return false;
        }
        let qualified = format!("{}.{}", enum_name.text, name.text);
        self.constructor_requires_expected(
            &ast::Ident {
                text: qualified,
                span: Span::new(enum_name.span.start, name.span.end),
            },
            args,
        )
    }

    /// Generic constructor parameter count and field templates, without
    /// producing diagnostics. `None` means the name is a function/class or
    /// does not denote a constructor.
    pub(super) fn constructor_inference_shape(&self, name: &str) -> Option<(usize, Vec<TypeId>)> {
        let variant = if let Some((enum_name, variant_name)) = name.split_once('.') {
            let enum_id = self.enums_by_name.get(enum_name).copied()?;
            self.find_variant(enum_id, variant_name)
                .map(|variant| (enum_id, variant))
        } else {
            self.option_variant(name)
        };
        if let Some((enum_id, variant)) = variant {
            return Some((
                self.enums[enum_id].type_params.len(),
                self.enums[enum_id].variants[variant as usize]
                    .fields
                    .iter()
                    .map(|field| field.ty)
                    .collect(),
            ));
        }
        self.structs_by_name
            .get(name)
            .map(|(struct_id, _)| {
                (
                    self.structs[*struct_id].type_params.len(),
                    self.structs[*struct_id]
                        .semantic_fields()
                        .iter()
                        .map(|field| field.ty)
                        .collect(),
                )
            })
            .or_else(|| {
                self.classes_by_name.get(name).map(|(class_id, _)| {
                    (
                        self.classes[*class_id].type_params.len(),
                        self.classes[*class_id]
                            .semantic_constructor()
                            .iter()
                            .map(|field| field.ty)
                            .collect(),
                    )
                })
            })
    }

    pub(super) fn mark_type_params(&self, ty: TypeId, bound: &mut [bool]) {
        match &self.types[ty] {
            Type::Param(index) => bound[index.into_raw() as usize] = true,
            Type::Struct(application) => {
                for arg in &self.struct_applications[*application].arguments {
                    self.mark_type_params(*arg, bound);
                }
            }
            Type::Class(application) => {
                for arg in &self.class_applications[*application].arguments {
                    self.mark_type_params(*arg, bound);
                }
            }
            Type::Enum(application) => {
                for arg in &self.enum_applications[*application].arguments {
                    self.mark_type_params(*arg, bound);
                }
            }
            Type::Interface(application) => {
                for arg in &self.interface_applications[*application].arguments {
                    self.mark_type_params(*arg, bound);
                }
            }
            Type::Tuple(args) => {
                for arg in args {
                    self.mark_type_params(*arg, bound);
                }
            }
            Type::Function(id) => {
                let function = &self.function_types[*id];
                for parameter in &function.parameter_types {
                    self.mark_type_params(*parameter, bound);
                }
                self.mark_type_params(function.return_type, bound);
            }
            Type::Ptr(pointee) => self.mark_type_params(*pointee, bound),
            Type::FunPtr(id) => {
                let function = &self.function_types[*id];
                for parameter in &function.parameter_types {
                    self.mark_type_params(*parameter, bound);
                }
                self.mark_type_params(function.return_type, bound);
            }
            _ => {}
        }
    }

    pub(super) fn unit_variant_from_field(
        &self,
        access: &ast::FieldAccess,
    ) -> Option<(hir::EnumId, u32)> {
        let ast::Expr::Var(enum_name) = access.receiver.as_ref() else {
            return None;
        };
        if self.scopes.lookup(&enum_name.text).is_some() || self.host_has_property(&enum_name.text)
        {
            return None;
        }
        let enum_id = self.enums_by_name.get(&enum_name.text).copied()?;
        let ast::FieldSelector::Name(variant_name) = &access.selector else {
            return None;
        };
        let variant = self.find_variant(enum_id, &variant_name.text)?;
        (!self.enums[enum_id].type_params.is_empty()
            && self.enums[enum_id].variants[variant as usize]
                .fields
                .is_empty())
        .then_some((enum_id, variant))
    }
}
