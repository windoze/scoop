//! Generic argument inference and structural type-parameter binding.

use super::*;

use crate::call_resolution::applicability::NominalApplicabilityInput;
use crate::call_resolution::candidates::{NominalConstructorSource, NominalConstructorView};
use crate::call_resolution::constraints::{
    ConstraintFailure, ConstraintFailureKind, ConstraintOrigin,
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
        debug_assert_eq!(argument_map.parameters.len(), arg_exprs.len());
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
        for input in &argument_map.parameters {
            let source_index = input.input.index();
            let parameter = view.value_parameters[input.parameter.index()].ty;
            let hint = self.try_substitute(parameter, &seed);
            if hint.is_none() && self.expr_requires_expected_type(&arg_exprs[source_index]) {
                continue;
            }
            lowered[source_index] =
                Some(self.lower_expr(&arg_exprs[source_index], &mut sinks[source_index], hint)?);
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
                        &arg_exprs[source_index],
                        &mut sinks[source_index],
                        None,
                    )?);
                }
                Err(failure) => {
                    self.diagnose_nominal_failure(view, &lowered, failure, span);
                    return None;
                }
            }
        };

        for input in &argument_map.parameters {
            let source_index = input.input.index();
            if lowered[source_index].is_some() {
                continue;
            }
            let parameter = view.value_parameters[input.parameter.index()].ty;
            let expected = self.instantiate_ty(parameter, &type_args);
            lowered[source_index] = Some(self.lower_expr(
                &arg_exprs[source_index],
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
                self.diagnose_nominal_failure(view, &lowered, failure, span);
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

    fn diagnose_nominal_failure(
        &mut self,
        view: &NominalConstructorView,
        arguments: &[Option<hir::Expr>],
        failure: ConstraintFailure,
        span: Span,
    ) {
        let parameter = |variable: crate::call_resolution::constraints::InferenceVariableId| {
            &view.owner_parameters[variable.group_index()]
        };
        match failure.kind {
            ConstraintFailureKind::Kind {
                variable,
                solution,
                required,
            } => {
                let parameter = parameter(variable);
                let required = match required {
                    hir::TypeParamKind::Any => return,
                    hir::TypeParamKind::Value => "value",
                    hir::TypeParamKind::Ref => "ref",
                };
                let found = self.type_name(solution);
                self.error(
                    span,
                    format!(
                        "type argument `{found}` for `{}` of {} must satisfy `{required}`",
                        parameter.name,
                        self.nominal_bound_target(view.target),
                    ),
                );
            }
            ConstraintFailureKind::InterfaceBound {
                variable,
                solution,
                required,
            } => {
                let parameter = parameter(variable);
                let found = self.type_name(solution);
                let required = self.type_name(required);
                self.error(
                    span,
                    format!(
                        "type argument `{found}` for `{}` of {} must satisfy interface upper bound `{required}`",
                        parameter.name,
                        self.nominal_bound_target(view.target),
                    ),
                );
            }
            ConstraintFailureKind::Relation { .. }
                if matches!(failure.origin, ConstraintOrigin::Argument(_)) =>
            {
                let ConstraintOrigin::Argument(input) = failure.origin else {
                    unreachable!()
                };
                let index = input.index();
                let argument = arguments[index]
                    .as_ref()
                    .expect("a relation failure has a typed source argument");
                let field = &view.value_parameters[index];
                let expected = self.type_name(field.ty);
                let found = self.type_name(argument.ty);
                self.error(
                    argument.span,
                    format!(
                        "argument for field `{}` of `{}` must be of type {expected}, found {found}",
                        field.name,
                        self.nominal_value_name(view.target),
                    ),
                );
            }
            ConstraintFailureKind::NoUniqueSolution { variable, .. }
            | ConstraintFailureKind::ConflictingExactBounds { variable, .. }
            | ConstraintFailureKind::UnresolvedTerm(
                crate::call_resolution::constraints::TypeTerm::Variable(variable),
            ) => {
                let parameter = parameter(variable);
                self.error(
                    span,
                    format!(
                        "cannot infer type argument `{}` for {}",
                        parameter.name,
                        self.nominal_inference_target(view.target),
                    ),
                );
            }
            _ => self.error(
                span,
                format!(
                    "constructor constraints for {} are not satisfied",
                    self.nominal_inference_target(view.target)
                ),
            ),
        }
    }

    fn nominal_bound_target(&self, target: NominalConstructorSource) -> String {
        match target {
            NominalConstructorSource::Struct(structure) => {
                format!("struct `{}`", self.structs[structure].name)
            }
            NominalConstructorSource::Class(class) => {
                format!("class `{}`", self.classes[class].name)
            }
            NominalConstructorSource::Variant { enumeration, .. } => {
                format!("enum `{}`", self.enums[enumeration].name)
            }
        }
    }

    fn nominal_inference_target(&self, target: NominalConstructorSource) -> String {
        match target {
            NominalConstructorSource::Variant {
                enumeration,
                variant,
            } => format!(
                "`{}.{}`",
                self.enums[enumeration].name,
                self.enums[enumeration].variants[variant as usize].name,
            ),
            _ => self.nominal_bound_target(target),
        }
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
        args: &[ast::Expr],
    ) -> bool {
        let Some((type_param_count, fields)) = self.constructor_inference_shape(&name.text) else {
            return false;
        };
        if type_param_count == 0 {
            return false;
        }
        let mut bound = vec![false; type_param_count];
        for (arg, field) in args.iter().zip(fields) {
            if !self.expr_requires_expected_type(arg) {
                self.mark_type_params(field, &mut bound);
            }
        }
        bound.iter().any(|bound| !bound)
    }

    pub(super) fn qualified_variant_requires_expected(
        &self,
        receiver: &ast::Expr,
        name: &ast::Ident,
        args: &[ast::Expr],
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
