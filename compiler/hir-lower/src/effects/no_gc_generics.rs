//! Generic GC-free precondition inference and call-site validation.

use scoop_ast::Span;
use scoop_hir as hir;

use crate::Lowerer;

#[derive(Debug, Clone, Copy)]
struct GenericCallSite {
    caller: hir::FunctionId,
    instantiation: hir::ResolvedGenericFunctionId,
    span: Span,
}

impl Lowerer {
    pub(super) fn validate_no_gc_instantiations(&mut self) {
        let call_sites = self.generic_call_sites();

        // A generic caller inherits the concrete GC-free preconditions of
        // every generic callee. Iterate to a fixed point so wrappers and
        // mutually recursive generic call graphs retain the full condition.
        loop {
            let mut additions = Vec::new();
            for call_site in &call_sites {
                let Some(&caller_generic) = self.generic_by_function.get(&call_site.caller) else {
                    continue;
                };
                let instantiation = &self.instantiations[call_site.instantiation];
                let callee_requirements = self.generic_functions[instantiation.generic]
                    .no_gc_type_params
                    .clone();
                for parameter in callee_requirements {
                    let argument = instantiation.type_args[parameter.into_raw() as usize];
                    let Some(mapped) = self.gc_free_requirements(argument) else {
                        continue;
                    };
                    for mapped_parameter in mapped {
                        if !self.generic_functions[caller_generic]
                            .no_gc_type_params
                            .contains(&mapped_parameter)
                        {
                            additions.push((caller_generic, mapped_parameter));
                        }
                    }
                }
            }
            if additions.is_empty() {
                break;
            }
            for (generic, parameter) in additions {
                let requirements = &mut self.generic_functions[generic].no_gc_type_params;
                if !requirements.contains(&parameter) {
                    requirements.push(parameter);
                }
            }
            for (_, generic) in self.generic_functions.iter_mut() {
                generic
                    .no_gc_type_params
                    .sort_by_key(|parameter| parameter.into_raw());
            }
        }

        // A ref-bound parameter can never satisfy a GC-free precondition.
        // Diagnose this on the generic definition even if no concrete caller
        // has instantiated it yet.
        let impossible_requirements: Vec<_> = self
            .generic_functions
            .iter()
            .flat_map(|(_, generic)| {
                let function = &self.functions[generic.function];
                generic
                    .no_gc_type_params
                    .iter()
                    .copied()
                    .filter(|parameter| {
                        function.type_params[parameter.into_raw() as usize].kind
                            == hir::TypeParamKind::Ref
                    })
                    .map(|parameter| {
                        (
                            generic.function,
                            parameter,
                            function.span,
                            function.name.clone(),
                            function.type_params[parameter.into_raw() as usize]
                                .name
                                .clone(),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        for (function, _, span, function_name, parameter_name) in impossible_requirements {
            self.current_file = self
                .function_files
                .get(&function)
                .copied()
                .unwrap_or(self.user_file_index);
            self.error(
                span,
                format!(
                    "generic function `{function_name}` cannot require ref-bound type parameter `{parameter_name}` to be GC-free"
                ),
            );
        }

        for call_site in call_sites {
            let instantiation = self.instantiations[call_site.instantiation].clone();
            let generic = self.generic_functions[instantiation.generic].clone();
            let requirements = generic.no_gc_type_params;
            if requirements.is_empty() {
                continue;
            }
            let callee = self.functions[generic.function].clone();
            let caller_requirements = self
                .generic_by_function
                .get(&call_site.caller)
                .map(|caller| self.generic_functions[*caller].no_gc_type_params.clone());
            self.current_file = self
                .function_files
                .get(&call_site.caller)
                .copied()
                .unwrap_or(self.user_file_index);

            for parameter in requirements {
                let index = parameter.into_raw() as usize;
                let argument = instantiation.type_args[index];
                let valid = match self.gc_free_requirements(argument) {
                    Some(mapped) if mapped.is_empty() => true,
                    Some(mapped) => caller_requirements
                        .as_ref()
                        .is_some_and(|caller| mapped.iter().all(|item| caller.contains(item))),
                    None => false,
                };
                if !valid {
                    self.error(
                        call_site.span,
                        format!(
                            "generic function `{}` requires type argument {} for `{}` to be GC-free",
                            callee.name,
                            self.type_name(argument),
                            callee.type_params[index].name
                        ),
                    );
                }
            }
        }
    }

    fn generic_call_sites(&self) -> Vec<GenericCallSite> {
        let mut out = Vec::new();
        for (caller, function) in self.functions.iter() {
            let hir::FunctionKind::User(body) = &function.kind else {
                continue;
            };
            self.collect_generic_calls_in_statements(caller, &body.statements, &mut out);
        }
        out
    }

    fn collect_generic_calls_in_statements(
        &self,
        caller: hir::FunctionId,
        statements: &[hir::Statement],
        out: &mut Vec<GenericCallSite>,
    ) {
        for statement in statements {
            match &statement.kind {
                hir::StatementKind::Expr(expr) | hir::StatementKind::Throw(expr) => {
                    self.collect_generic_calls_in_expr(caller, expr, out);
                }
                hir::StatementKind::LocalFunction(_) => {}
                hir::StatementKind::Return { value } => {
                    if let Some(value) = value {
                        self.collect_generic_calls_in_expr(caller, value, out);
                    }
                }
                hir::StatementKind::ValDecl { pattern, init } => {
                    self.collect_generic_calls_in_pattern(caller, pattern, out);
                    self.collect_generic_calls_in_expr(caller, init, out);
                }
                hir::StatementKind::Assign { target, value } => {
                    match target {
                        hir::AssignTarget::Local(_) | hir::AssignTarget::Global(_) => {}
                        hir::AssignTarget::Index { array, index } => {
                            self.collect_generic_calls_in_expr(caller, array, out);
                            self.collect_generic_calls_in_expr(caller, index, out);
                        }
                        hir::AssignTarget::Field { receiver, .. } => {
                            self.collect_generic_calls_in_expr(caller, receiver, out);
                        }
                    }
                    self.collect_generic_calls_in_expr(caller, value, out);
                }
                hir::StatementKind::If {
                    cond,
                    then_body,
                    else_body,
                } => {
                    self.collect_generic_calls_in_expr(caller, cond, out);
                    self.collect_generic_calls_in_statements(caller, then_body, out);
                    if let Some(else_body) = else_body {
                        self.collect_generic_calls_in_statements(caller, else_body, out);
                    }
                }
                hir::StatementKind::While { cond, body } => {
                    self.collect_generic_calls_in_expr(caller, cond, out);
                    self.collect_generic_calls_in_statements(caller, body, out);
                }
                hir::StatementKind::When(when) => {
                    self.collect_generic_calls_in_expr(caller, &when.subject, out);
                    for arm in &when.arms {
                        self.collect_generic_calls_in_pattern(caller, &arm.pattern, out);
                        if let Some(guard) = &arm.guard {
                            self.collect_generic_calls_in_expr(caller, guard, out);
                        }
                        self.collect_generic_calls_in_statements(caller, &arm.body, out);
                    }
                    if let Some(else_body) = &when.else_body {
                        self.collect_generic_calls_in_statements(caller, else_body, out);
                    }
                }
                hir::StatementKind::Try(try_) => {
                    self.collect_generic_calls_in_statements(caller, &try_.body, out);
                    for catch in &try_.catches {
                        self.collect_generic_calls_in_statements(caller, &catch.body, out);
                    }
                    if let Some(finally_body) = &try_.finally_body {
                        self.collect_generic_calls_in_statements(caller, finally_body, out);
                    }
                }
            }
        }
    }

    fn collect_generic_calls_in_pattern(
        &self,
        caller: hir::FunctionId,
        pattern: &hir::Pattern,
        out: &mut Vec<GenericCallSite>,
    ) {
        match pattern {
            hir::Pattern::Literal(expr) => self.collect_generic_calls_in_expr(caller, expr, out),
            hir::Pattern::Variant { fields, .. } | hir::Pattern::Struct { fields, .. } => {
                for (_, pattern) in fields {
                    self.collect_generic_calls_in_pattern(caller, pattern, out);
                }
            }
            hir::Pattern::Tuple(elements) => {
                for pattern in elements {
                    self.collect_generic_calls_in_pattern(caller, pattern, out);
                }
            }
            hir::Pattern::Binding { .. } | hir::Pattern::Wildcard => {}
        }
    }

    fn collect_generic_calls_in_expr(
        &self,
        caller: hir::FunctionId,
        expr: &hir::Expr,
        out: &mut Vec<GenericCallSite>,
    ) {
        use hir::ExprKind;

        let mut record = |callable: hir::Callable| {
            if let hir::Callable::Generic(instantiation) = callable {
                out.push(GenericCallSite {
                    caller,
                    instantiation,
                    span: expr.span,
                });
            }
        };
        match &expr.kind {
            ExprKind::StringLiteral(_)
            | ExprKind::IntLiteral(_)
            | ExprKind::BoolLiteral(_)
            | ExprKind::UnitLiteral
            | ExprKind::Local(_)
            | ExprKind::GlobalRead(_)
            | ExprKind::Capture(_)
            | ExprKind::Lambda(_)
            | ExprKind::AnonymousFunction(_)
            | ExprKind::NoneLiteral
            | ExprKind::AddressOf(_)
            | ExprKind::SizeOf(_)
            | ExprKind::AlignOf(_)
            | ExprKind::FunPtrNull
            | ExprKind::FunctionAddress(_) => {}
            ExprKind::TupleLiteral(elements) | ExprKind::ArrayLiteral(elements) => {
                for element in elements {
                    self.collect_generic_calls_in_expr(caller, element, out);
                }
            }
            ExprKind::StructInit { args, .. }
            | ExprKind::ClassInit { args, .. }
            | ExprKind::VariantConstruct { args, .. } => {
                for arg in args {
                    self.collect_generic_calls_in_expr(caller, arg, out);
                }
            }
            ExprKind::CallableReference(reference) => {
                let reference = &self.callable_references[*reference];
                match &reference.target {
                    hir::CallableReferenceTarget::Named(_)
                    | hir::CallableReferenceTarget::Local { .. } => {}
                    hir::CallableReferenceTarget::BoundMember { receiver, .. }
                    | hir::CallableReferenceTarget::BoundExtension { receiver, .. } => {
                        self.collect_generic_calls_in_expr(caller, receiver, out);
                    }
                }
            }
            ExprKind::FunctionCoercion { source, .. }
            | ExprKind::PtrFromUInt(source)
            | ExprKind::PtrToUInt(source)
            | ExprKind::PtrCast(source)
            | ExprKind::Box(source)
            | ExprKind::Unbox(source)
            | ExprKind::IsInstance {
                operand: source, ..
            }
            | ExprKind::Cast {
                operand: source, ..
            }
            | ExprKind::ArrayLen(source)
            | ExprKind::ArrayClone(source)
            | ExprKind::Unary {
                operand: source, ..
            }
            | ExprKind::SomeWrap(source)
            | ExprKind::IsSome(source)
            | ExprKind::Unwrap {
                operand: source, ..
            } => self.collect_generic_calls_in_expr(caller, source, out),
            ExprKind::PtrLoad { pointer, offset } => {
                self.collect_generic_calls_in_expr(caller, pointer, out);
                if let Some(offset) = offset {
                    self.collect_generic_calls_in_expr(caller, offset, out);
                }
            }
            ExprKind::PtrStore {
                pointer,
                offset,
                value,
            } => {
                self.collect_generic_calls_in_expr(caller, pointer, out);
                if let Some(offset) = offset {
                    self.collect_generic_calls_in_expr(caller, offset, out);
                }
                self.collect_generic_calls_in_expr(caller, value, out);
            }
            ExprKind::PtrOffset {
                pointer, offset, ..
            }
            | ExprKind::Index {
                receiver: pointer,
                index: offset,
            }
            | ExprKind::Binary {
                lhs: pointer,
                rhs: offset,
                ..
            } => {
                self.collect_generic_calls_in_expr(caller, pointer, out);
                self.collect_generic_calls_in_expr(caller, offset, out);
            }
            ExprKind::FieldAccess { receiver, .. } => {
                self.collect_generic_calls_in_expr(caller, receiver, out);
            }
            ExprKind::MethodCall {
                receiver,
                callee,
                args,
            } => {
                record(*callee);
                self.collect_generic_calls_in_expr(caller, receiver, out);
                for arg in args {
                    self.collect_generic_calls_in_expr(caller, arg, out);
                }
            }
            ExprKind::Call { callee, args } => {
                record(*callee);
                for arg in args {
                    self.collect_generic_calls_in_expr(caller, arg, out);
                }
            }
            ExprKind::LocalFunctionCall {
                callee,
                captures,
                args,
                ..
            } => {
                record(*callee);
                for value in captures.iter().chain(args) {
                    self.collect_generic_calls_in_expr(caller, value, out);
                }
            }
            ExprKind::CallableCall { callee, args, .. } => {
                self.collect_generic_calls_in_expr(caller, callee, out);
                for arg in args {
                    self.collect_generic_calls_in_expr(caller, arg, out);
                }
            }
        }
    }
}
