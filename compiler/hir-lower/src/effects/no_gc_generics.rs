//! Generic GC-free precondition inference and call-site validation.

use scoop_ast::Span;
use scoop_hir as hir;

use crate::Lowerer;

#[derive(Debug, Clone)]
pub(super) struct GenericCallSite {
    pub(super) caller: hir::FunctionId,
    pub(super) callee: hir::FunctionId,
    /// Complete declaration-parameter to application-argument relation. It
    /// is built from the exact callable application variants; consumers do
    /// not split or reconstruct an argument vector.
    pub(super) arguments: Vec<(hir::TypeParamId, hir::TypeId)>,
    pub(super) span: Span,
}

impl GenericCallSite {
    fn argument(&self, parameter: hir::TypeParamId) -> hir::TypeId {
        self.arguments
            .iter()
            .find_map(|(candidate, argument)| (*candidate == parameter).then_some(*argument))
            .expect("every parameterized call application binds every declaration parameter")
    }
}

impl Lowerer {
    pub(super) fn validate_no_gc_instantiations(&mut self) {
        let call_sites = self.generic_call_sites();

        // A parameterized caller inherits the concrete GC-free preconditions
        // of every parameterized callee. Iterate to a fixed point so wrappers
        // and mutually recursive call graphs retain the full condition.
        loop {
            let mut additions = Vec::new();
            for call_site in &call_sites {
                if self.functions[call_site.caller].type_param_count() == 0 {
                    continue;
                }
                let callee_requirements =
                    self.function_no_gc_requirements(call_site.callee).to_vec();
                for parameter in callee_requirements {
                    let argument = call_site.argument(parameter);
                    let Some(mapped) = self.gc_free_requirements(argument) else {
                        continue;
                    };
                    for mapped_parameter in mapped {
                        if !self
                            .function_no_gc_requirements(call_site.caller)
                            .contains(&mapped_parameter)
                        {
                            additions.push((call_site.caller, mapped_parameter));
                        }
                    }
                }
            }
            if additions.is_empty() {
                break;
            }
            for (function, parameter) in additions {
                self.add_function_no_gc_requirement(function, parameter);
            }
        }

        // A ref-bound parameter can never satisfy a GC-free precondition.
        // Diagnose this on the exact callable declaration even if no concrete
        // caller has instantiated it yet.
        let impossible_requirements: Vec<_> = self
            .functions
            .iter()
            .flat_map(|(function_id, function)| {
                self.function_no_gc_requirements(function_id)
                    .iter()
                    .copied()
                    .filter(|parameter| {
                        function.type_param(*parameter).kind() == hir::TypeParamKind::Ref
                    })
                    .map(|parameter| {
                        (
                            function_id,
                            parameter,
                            function.span,
                            function.name.clone(),
                            function.type_param(parameter).name.clone(),
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
            let requirements = self.function_no_gc_requirements(call_site.callee).to_vec();
            if requirements.is_empty() {
                continue;
            }
            let callee = self.functions[call_site.callee].clone();
            let caller_requirements = (self.functions[call_site.caller].type_param_count() != 0)
                .then(|| self.function_no_gc_requirements(call_site.caller).to_vec());
            self.current_file = self
                .function_files
                .get(&call_site.caller)
                .copied()
                .unwrap_or(self.user_file_index);

            for parameter in requirements {
                let argument = call_site.argument(parameter);
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
                            callee.type_param(parameter).name
                        ),
                    );
                }
            }
        }
    }

    pub(super) fn set_function_no_gc_requirements(
        &mut self,
        function: hir::FunctionId,
        mut requirements: Vec<hir::TypeParamId>,
    ) {
        requirements.sort_by_key(|parameter| parameter.into_raw());
        requirements.dedup();
        match self.functions[function].genericity.clone() {
            hir::FunctionGenericity::Plain => {
                assert!(
                    requirements.is_empty(),
                    "a parameter-free function cannot have type-parameter requirements"
                );
            }
            hir::FunctionGenericity::Generic { definition, .. } => {
                self.generic_functions[definition].no_gc_type_params = requirements;
            }
            hir::FunctionGenericity::OwnerParameterizedMethod { .. } => {
                let hir::FunctionGenericity::OwnerParameterizedMethod {
                    no_gc_type_params, ..
                } = &mut self.functions[function].genericity
                else {
                    unreachable!("the matched genericity remains stable")
                };
                *no_gc_type_params = requirements;
            }
            hir::FunctionGenericity::GenericMethod { definition, .. } => {
                self.generic_methods[definition].no_gc_type_params = requirements;
            }
        }
    }

    fn function_no_gc_requirements(&self, function: hir::FunctionId) -> &[hir::TypeParamId] {
        match &self.functions[function].genericity {
            hir::FunctionGenericity::Plain => &[],
            hir::FunctionGenericity::Generic { definition, .. } => {
                &self.generic_functions[*definition].no_gc_type_params
            }
            hir::FunctionGenericity::OwnerParameterizedMethod {
                no_gc_type_params, ..
            } => no_gc_type_params,
            hir::FunctionGenericity::GenericMethod { definition, .. } => {
                &self.generic_methods[*definition].no_gc_type_params
            }
        }
    }

    fn add_function_no_gc_requirement(
        &mut self,
        function: hir::FunctionId,
        parameter: hir::TypeParamId,
    ) {
        let mut requirements = self.function_no_gc_requirements(function).to_vec();
        if !requirements.contains(&parameter) {
            requirements.push(parameter);
            self.set_function_no_gc_requirements(function, requirements);
        }
    }

    pub(super) fn generic_call_sites(&self) -> Vec<GenericCallSite> {
        let mut out = Vec::new();
        for (caller, function) in self.functions.iter() {
            let hir::FunctionKind::User(body) = &function.kind else {
                continue;
            };
            self.collect_generic_calls_in_statements(caller, &body.statements, &mut out);
        }
        out
    }

    fn generic_call_site(
        &self,
        caller: hir::FunctionId,
        callable: hir::Callable,
        span: Span,
    ) -> Option<GenericCallSite> {
        let (callee, arguments) = match callable {
            hir::Callable::Function(_) => return None,
            hir::Callable::Generic(application) => {
                let application = &self.instantiations[application];
                let generic = &self.generic_functions[application.generic];
                let hir::FunctionGenericity::Generic { parameters, .. } =
                    &self.functions[generic.function].genericity
                else {
                    unreachable!("a generic application names a generic function declaration")
                };
                assert_eq!(parameters.len(), application.type_args.len());
                let arguments = parameters
                    .iter()
                    .zip(application.type_args.iter().copied())
                    .map(|(parameter, argument)| (parameter.id, argument))
                    .collect();
                (generic.function, arguments)
            }
            hir::Callable::Method(application) => {
                let application = &self.method_applications[application];
                let hir::FunctionGenericity::OwnerParameterizedMethod {
                    owner_parameters, ..
                } = &self.functions[application.function].genericity
                else {
                    // Parameter-free ordinary methods have no generic GC-free
                    // preconditions and therefore need no call-site record.
                    return None;
                };
                let owner_arguments = self.method_owner_arguments(application.owner);
                assert_eq!(owner_parameters.len(), owner_arguments.len());
                let arguments = owner_parameters
                    .iter()
                    .zip(owner_arguments.iter().copied())
                    .map(|(parameter, argument)| (parameter.id, argument))
                    .collect();
                (application.function, arguments)
            }
            hir::Callable::GenericMethod(application) => {
                let application = &self.generic_method_applications[application];
                let method = &self.generic_methods[application.method];
                let hir::FunctionGenericity::GenericMethod {
                    owner_parameters,
                    method_parameters,
                    ..
                } = &self.functions[method.function].genericity
                else {
                    unreachable!("a generic method application names a generic method declaration")
                };
                let owner_arguments = self.generic_method_owner_arguments(application.owner);
                assert_eq!(owner_parameters.len(), owner_arguments.len());
                assert_eq!(method_parameters.len(), application.method_arguments.len());
                let mut arguments = owner_parameters
                    .iter()
                    .zip(owner_arguments.iter().copied())
                    .map(|(parameter, argument)| (parameter.id, argument))
                    .collect::<Vec<_>>();
                arguments.extend(
                    method_parameters
                        .iter()
                        .zip(application.method_arguments.iter().copied())
                        .map(|(parameter, argument)| (parameter.id, argument)),
                );
                (method.function, arguments)
            }
        };
        Some(GenericCallSite {
            caller,
            callee,
            arguments,
            span,
        })
    }

    fn generic_method_owner_arguments(&self, owner: hir::GenericMethodOwner) -> &[hir::TypeId] {
        match owner {
            hir::GenericMethodOwner::Class(id) => &self.class_applications[id].arguments,
            hir::GenericMethodOwner::Struct(id) => &self.struct_applications[id].arguments,
            hir::GenericMethodOwner::Enum(id) => &self.enum_applications[id].arguments,
        }
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
            hir::Pattern::Literal { value, .. } => {
                self.collect_generic_calls_in_expr(caller, value, out)
            }
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
            if let Some(call_site) = self.generic_call_site(caller, callable, expr.span) {
                out.push(call_site);
            }
        };
        match &expr.kind {
            ExprKind::StringLiteral(_)
            | ExprKind::IntLiteral(_)
            | ExprKind::BoolLiteral(_)
            | ExprKind::UnitLiteral
            | ExprKind::Local(_)
            | ExprKind::ConstructorParam(_)
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
                    hir::CallableReferenceTarget::Named(callee) => record(*callee),
                    hir::CallableReferenceTarget::Local { callee, .. } => record(*callee),
                    hir::CallableReferenceTarget::BoundMember { receiver, callee } => {
                        if let hir::MethodCallee::Callable(callee) = callee {
                            record(*callee);
                        }
                        self.collect_generic_calls_in_expr(caller, receiver, out);
                    }
                    hir::CallableReferenceTarget::BoundExtension { receiver, callee } => {
                        record(*callee);
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
                if let hir::MethodCallee::Callable(callee) = callee {
                    record(*callee);
                }
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
            ExprKind::ForeignCallbackRegister { closure, .. } => {
                self.collect_generic_calls_in_expr(caller, closure, out);
            }
            ExprKind::ForeignCallbackOperation { callback, .. } => {
                self.collect_generic_calls_in_expr(caller, callback, out);
            }
        }
    }
}
