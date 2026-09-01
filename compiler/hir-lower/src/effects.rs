//! Verification of M12 safety and `@NoGC` effects over resolved HIR.

use std::collections::HashSet;

use scoop_ast::Span;
use scoop_hir as hir;

use crate::Lowerer;

mod generic_recursion;
mod no_gc_generics;

/// Type arguments in a generic aggregate are expressed in the caller's
/// parameter namespace. Chaining environments lets GC-freedom analysis
/// substitute nested generic fields without interning synthetic HIR types.
struct TypeEnvironment<'a> {
    bindings: Vec<(hir::TypeParamId, hir::TypeId)>,
    parent: Option<&'a TypeEnvironment<'a>>,
}

impl<'a> TypeEnvironment<'a> {
    fn resolve(
        &'a self,
        parameter: hir::TypeParamId,
    ) -> Option<(hir::TypeId, Option<&'a TypeEnvironment<'a>>)> {
        if let Some(argument) = self
            .bindings
            .iter()
            .find_map(|(candidate, argument)| (*candidate == parameter).then_some(*argument))
        {
            return Some((argument, self.parent));
        }
        self.parent?.resolve(parameter)
    }
}

impl Lowerer {
    pub(crate) fn check_no_gc_types(&mut self) {
        let mut seen = HashSet::new();
        let concrete_enums = self
            .enums
            .iter()
            .filter(|(_, declaration)| declaration.no_gc && declaration.type_params.is_empty())
            .map(|(_, declaration)| {
                (
                    declaration.name.clone(),
                    declaration.span,
                    declaration.variants.iter().all(|variant| {
                        variant.fields.iter().all(|field| self.is_gc_free(field.ty))
                    }),
                )
            })
            .collect::<Vec<_>>();
        for (name, span, gc_free) in concrete_enums {
            if !gc_free {
                self.error(
                    span,
                    format!(
                        "`@NoGC` enum specialization `{name}` is not GC-free because it directly or indirectly contains a ref type"
                    ),
                );
            }
        }
        let types =
            self.types
                .iter()
                .filter_map(|(ty, kind)| match kind {
                    hir::Type::Struct(application) => {
                        let id = self.struct_applications[*application].template;
                        self.structs[id].attributes.no_gc.then_some((
                            ty,
                            "struct",
                            id.into_raw().into_u32(),
                            self.structs[id].span,
                        ))
                    }
                    hir::Type::Enum(application) => {
                        let id = self.enum_applications[*application].template;
                        (self.enums[id].no_gc && !self.enums[id].type_params.is_empty())
                            .then_some((ty, "enum", id.into_raw().into_u32(), self.enums[id].span))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
        for (ty, kind, raw_id, span) in types {
            if self.type_contains_param(ty) {
                continue;
            }
            let name = self.type_name(ty);
            if !seen.insert((kind, raw_id, name.clone())) {
                continue;
            }
            if !self.is_gc_free(ty) {
                self.error(
                    span,
                    format!(
                        "`@NoGC` {kind} specialization `{name}` is not GC-free because it directly or indirectly contains a ref type"
                    ),
                );
            }
        }
    }

    pub(crate) fn check_no_gc_functions(&mut self) {
        let functions: Vec<_> = self
            .functions
            .iter()
            .filter_map(|(id, function)| {
                (function.attributes.gc_effect == hir::GcEffect::NoGc).then_some(id)
            })
            .collect();
        for id in functions {
            self.current_file = self
                .function_files
                .get(&id)
                .copied()
                .unwrap_or(self.user_file_index);
            let function = self.functions[id].clone();
            if function.is_suspend {
                continue; // The annotation combination already owns this error.
            }
            if matches!(function.kind, hir::FunctionKind::Intrinsic(_)) {
                // Core intrinsic signatures and effects are validated against
                // the intrinsic registry before this whole-program pass.
                continue;
            }
            let mut requirements = HashSet::new();
            if let hir::FunctionKind::Extern(extern_id) = function.kind {
                let extern_ = self.extern_functions[extern_id].clone();
                if extern_.abi == hir::ExternAbi::C {
                    // The C-FFI-safe classifier is the stronger signature
                    // check and already proves every boundary value GC-free.
                    continue;
                }
                for (index, ty) in extern_.params.into_iter().enumerate() {
                    if !self.is_gc_free(ty) {
                        self.error(
                            function.span,
                            format!(
                                "`@NoGC` extern function `{}` has non-GC-free parameter {} of type {}",
                                function.name,
                                index + 1,
                                self.type_name(ty)
                            ),
                        );
                    }
                }
                if !self.is_gc_free(extern_.return_type) {
                    self.error(
                        function.span,
                        format!(
                            "`@NoGC` extern function `{}` has non-GC-free return type {}",
                            function.name,
                            self.type_name(extern_.return_type)
                        ),
                    );
                }
                continue;
            }
            for param in &function.params {
                match self.gc_free_requirements(param.ty) {
                    Some(required) => requirements.extend(required),
                    None => {
                        self.error(
                            function.span,
                            format!(
                                "`@NoGC` function `{}` has non-GC-free parameter `{}` of type {}",
                                function.name,
                                param.name,
                                self.type_name(param.ty)
                            ),
                        );
                    }
                }
            }
            match self.gc_free_requirements(function.return_ty) {
                Some(required) => requirements.extend(required),
                None => {
                    self.error(
                        function.span,
                        format!(
                            "`@NoGC` function `{}` has non-GC-free return type {}",
                            function.name,
                            self.type_name(function.return_ty)
                        ),
                    );
                }
            }
            let hir::FunctionKind::User(body) = function.kind else {
                continue;
            };
            let parameter_locals: HashSet<_> =
                function.params.iter().map(|param| param.local).collect();
            for (local_id, local) in body.locals.iter() {
                if parameter_locals.contains(&local_id) {
                    continue;
                }
                match self.gc_free_requirements(local.ty) {
                    Some(required) => requirements.extend(required),
                    None => {
                        self.error(
                            function.span,
                            format!(
                                "`@NoGC` function `{}` has local `{}` of non-GC-free type {}",
                                function.name,
                                local.name,
                                self.type_name(local.ty)
                            ),
                        );
                    }
                }
            }
            let mut violations = Vec::new();
            self.collect_no_gc_statement_violations(
                &body.statements,
                &mut violations,
                &mut requirements,
            );
            for (span, message) in violations {
                self.error(span, message);
            }
            let mut requirements: Vec<_> = requirements.into_iter().collect();
            requirements.sort_by_key(|parameter| parameter.into_raw());
            self.set_function_no_gc_requirements(id, requirements);
        }

        self.validate_no_gc_instantiations();

        let safe_functions: Vec<_> = self
            .functions
            .iter()
            .filter_map(|(id, function)| {
                (function.attributes.safety == hir::Safety::Safe).then_some(id)
            })
            .collect();
        for id in safe_functions {
            self.current_file = self
                .function_files
                .get(&id)
                .copied()
                .unwrap_or(self.user_file_index);
            let function = self.functions[id].clone();
            if let hir::FunctionKind::Extern(extern_id) = function.kind {
                let extern_ = self.extern_functions[extern_id].clone();
                for (index, ty) in extern_.params.into_iter().enumerate() {
                    if self.requires_unsafe_use(ty) {
                        self.error(
                            function.span,
                            format!(
                                "safe extern function `{}` exposes `@InteriorMutable` parameter {}",
                                function.name,
                                index + 1
                            ),
                        );
                    }
                }
                if self.requires_unsafe_use(extern_.return_type) {
                    self.error(
                        function.span,
                        format!(
                            "safe extern function `{}` exposes an `@InteriorMutable` return type",
                            function.name
                        ),
                    );
                }
                continue;
            }
            for param in &function.params {
                if self.requires_unsafe_use(param.ty) {
                    self.error(
                        function.span,
                        format!(
                            "safe function `{}` exposes `@InteriorMutable` parameter `{}`",
                            function.name, param.name
                        ),
                    );
                }
            }
            if self.requires_unsafe_use(function.return_ty) {
                self.error(
                    function.span,
                    format!(
                        "safe function `{}` exposes an `@InteriorMutable` return type",
                        function.name
                    ),
                );
            }
        }
    }

    pub(crate) fn is_gc_free(&self, ty: hir::TypeId) -> bool {
        self.gc_free_requirements(ty)
            .is_some_and(|requirements| requirements.is_empty())
    }

    fn gc_free_requirements(&self, ty: hir::TypeId) -> Option<HashSet<hir::TypeParamId>> {
        self.gc_free_requirements_inner(ty, None, &mut HashSet::new())
    }

    fn gc_free_requirements_inner(
        &self,
        ty: hir::TypeId,
        environment: Option<&TypeEnvironment<'_>>,
        visiting: &mut HashSet<hir::TypeId>,
    ) -> Option<HashSet<hir::TypeParamId>> {
        match &self.types[ty] {
            hir::Type::Unit
            | hir::Type::Int
            | hir::Type::UInt
            | hir::Type::Boolean
            | hir::Type::Ptr(_)
            | hir::Type::FunPtr(_) => Some(HashSet::new()),
            hir::Type::String
            | hir::Type::Class(..)
            | hir::Type::Interface(_)
            | hir::Type::Any
            | hir::Type::Function(_) => None,
            hir::Type::Tuple(elements) => {
                let mut requirements = HashSet::new();
                for element in elements {
                    requirements.extend(self.gc_free_requirements_inner(
                        *element,
                        environment,
                        visiting,
                    )?);
                }
                Some(requirements)
            }
            hir::Type::Param(index) => match environment {
                Some(environment) => {
                    let (argument, parent) = environment
                        .resolve(*index)
                        .expect("the complete type environment binds every referenced parameter");
                    self.gc_free_requirements_inner(argument, parent, visiting)
                }
                None => Some(HashSet::from([*index])),
            },
            hir::Type::Struct(application) => {
                let application = &self.struct_applications[*application];
                let id = application.template;
                if !visiting.insert(ty) {
                    return None;
                }
                let nested = TypeEnvironment {
                    bindings: self.structs[id]
                        .type_params
                        .iter()
                        .zip(application.arguments.iter().copied())
                        .map(|(parameter, argument)| (parameter.id, argument))
                        .collect(),
                    parent: environment,
                };
                let mut requirements = HashSet::new();
                for field in self.structs[id].semantic_fields() {
                    let Some(required) =
                        self.gc_free_requirements_inner(field.ty, Some(&nested), visiting)
                    else {
                        visiting.remove(&ty);
                        return None;
                    };
                    requirements.extend(required);
                }
                visiting.remove(&ty);
                Some(requirements)
            }
            hir::Type::Enum(application) => {
                let application = &self.enum_applications[*application];
                let id = application.template;
                if !visiting.insert(ty) {
                    return None;
                }
                let nested = TypeEnvironment {
                    bindings: self.enums[id]
                        .type_params
                        .iter()
                        .zip(application.arguments.iter().copied())
                        .map(|(parameter, argument)| (parameter.id, argument))
                        .collect(),
                    parent: environment,
                };
                let mut requirements = HashSet::new();
                for field in self.enums[id]
                    .variants
                    .iter()
                    .flat_map(|variant| &variant.fields)
                {
                    let Some(required) =
                        self.gc_free_requirements_inner(field.ty, Some(&nested), visiting)
                    else {
                        visiting.remove(&ty);
                        return None;
                    };
                    requirements.extend(required);
                }
                visiting.remove(&ty);
                Some(requirements)
            }
        }
    }

    pub(crate) fn requires_unsafe_use(&self, ty: hir::TypeId) -> bool {
        self.requires_unsafe_use_inner(ty, None, &mut HashSet::new())
    }

    fn requires_unsafe_use_inner(
        &self,
        ty: hir::TypeId,
        environment: Option<&TypeEnvironment<'_>>,
        visiting: &mut HashSet<hir::TypeId>,
    ) -> bool {
        match &self.types[ty] {
            hir::Type::Struct(application) => {
                let application = &self.struct_applications[*application];
                let id = application.template;
                if self.structs[id].attributes.interior_mutable {
                    return true;
                }
                if !visiting.insert(ty) {
                    return false;
                }
                let nested = TypeEnvironment {
                    bindings: self.structs[id]
                        .type_params
                        .iter()
                        .zip(application.arguments.iter().copied())
                        .map(|(parameter, argument)| (parameter.id, argument))
                        .collect(),
                    parent: environment,
                };
                let result = self.structs[id]
                    .semantic_fields()
                    .iter()
                    .any(|field| self.requires_unsafe_use_inner(field.ty, Some(&nested), visiting));
                visiting.remove(&ty);
                result
            }
            hir::Type::Enum(application) => {
                let application = &self.enum_applications[*application];
                let id = application.template;
                if !visiting.insert(ty) {
                    return false;
                }
                let nested = TypeEnvironment {
                    bindings: self.enums[id]
                        .type_params
                        .iter()
                        .zip(application.arguments.iter().copied())
                        .map(|(parameter, argument)| (parameter.id, argument))
                        .collect(),
                    parent: environment,
                };
                let result = self.enums[id].variants.iter().any(|variant| {
                    variant.fields.iter().any(|field| {
                        self.requires_unsafe_use_inner(field.ty, Some(&nested), visiting)
                    })
                });
                visiting.remove(&ty);
                result
            }
            hir::Type::Tuple(elements) => elements
                .iter()
                .any(|ty| self.requires_unsafe_use_inner(*ty, environment, visiting)),
            hir::Type::Param(index) => environment.is_some_and(|environment| {
                let (argument, parent) = environment
                    .resolve(*index)
                    .expect("the complete type environment binds every referenced parameter");
                self.requires_unsafe_use_inner(argument, parent, visiting)
            }),
            _ => false,
        }
    }

    pub(crate) fn require_unsafe_type_use(&mut self, ty: hir::TypeId, span: Span) -> bool {
        if !self.requires_unsafe_use(ty) {
            return true;
        }
        let safety = self
            .safety_contexts
            .last()
            .copied()
            .expect("the safety context stack is initialized");
        if safety == hir::Safety::Unsafe {
            return true;
        }
        self.error(
            span,
            format!(
                "value of type {} contains `@InteriorMutable` state and requires an unsafe context",
                self.type_name(ty)
            ),
        );
        false
    }

    fn collect_no_gc_statement_violations(
        &self,
        statements: &[hir::Statement],
        out: &mut Vec<(Span, String)>,
        requirements: &mut HashSet<hir::TypeParamId>,
    ) {
        for statement in statements {
            match &statement.kind {
                hir::StatementKind::Expr(expr) => {
                    self.collect_no_gc_expr_violations(expr, out, requirements)
                }
                hir::StatementKind::LocalFunction(_) => {}
                hir::StatementKind::Return { value } => {
                    if let Some(value) = value {
                        self.collect_no_gc_expr_violations(value, out, requirements);
                    }
                }
                hir::StatementKind::ValDecl { init, .. } => {
                    self.collect_no_gc_expr_violations(init, out, requirements)
                }
                hir::StatementKind::Assign { target, value } => {
                    match target {
                        hir::AssignTarget::Local(_) | hir::AssignTarget::Global(_) => {}
                        hir::AssignTarget::Index { array, index } => {
                            out.push((
                                statement.span,
                                "array assignment is not allowed in `@NoGC` code".to_string(),
                            ));
                            self.collect_no_gc_expr_violations(array, out, requirements);
                            self.collect_no_gc_expr_violations(index, out, requirements);
                        }
                        hir::AssignTarget::Field { receiver, .. } => {
                            out.push((
                                statement.span,
                                "managed field assignment is not allowed in `@NoGC` code"
                                    .to_string(),
                            ));
                            self.collect_no_gc_expr_violations(receiver, out, requirements);
                        }
                    }
                    self.collect_no_gc_expr_violations(value, out, requirements);
                }
                hir::StatementKind::If {
                    cond,
                    then_body,
                    else_body,
                } => {
                    self.collect_no_gc_expr_violations(cond, out, requirements);
                    self.collect_no_gc_statement_violations(then_body, out, requirements);
                    if let Some(else_body) = else_body {
                        self.collect_no_gc_statement_violations(else_body, out, requirements);
                    }
                }
                hir::StatementKind::While { cond, body } => {
                    self.collect_no_gc_expr_violations(cond, out, requirements);
                    self.collect_no_gc_statement_violations(body, out, requirements);
                }
                hir::StatementKind::When(when) => {
                    self.collect_no_gc_expr_violations(&when.subject, out, requirements);
                    for arm in &when.arms {
                        if let Some(guard) = &arm.guard {
                            self.collect_no_gc_expr_violations(guard, out, requirements);
                        }
                        self.collect_no_gc_statement_violations(&arm.body, out, requirements);
                    }
                    if let Some(else_body) = &when.else_body {
                        self.collect_no_gc_statement_violations(else_body, out, requirements);
                    }
                }
                hir::StatementKind::Try(_) => out.push((
                    statement.span,
                    "`try`/`catch` is not allowed in `@NoGC` code".to_string(),
                )),
                hir::StatementKind::Throw(expr) => {
                    out.push((
                        statement.span,
                        "`throw` is not allowed in `@NoGC` code".to_string(),
                    ));
                    self.collect_no_gc_expr_violations(expr, out, requirements);
                }
            }
        }
    }

    fn collect_no_gc_expr_violations(
        &self,
        expr: &hir::Expr,
        out: &mut Vec<(Span, String)>,
        requirements: &mut HashSet<hir::TypeParamId>,
    ) {
        use hir::ExprKind;
        match self.gc_free_requirements(expr.ty) {
            Some(required) => requirements.extend(required),
            None => {
                out.push((
                    expr.span,
                    format!(
                        "value of non-GC-free type {} is not allowed in `@NoGC` code",
                        self.type_name(expr.ty)
                    ),
                ));
            }
        }
        match &expr.kind {
            ExprKind::StringLiteral(_) => out.push((
                expr.span,
                "string literals are not allowed in `@NoGC` code".to_string(),
            )),
            ExprKind::IntLiteral(_)
            | ExprKind::BoolLiteral(_)
            | ExprKind::UnitLiteral
            | ExprKind::Local(_)
            | ExprKind::ConstructorParam(_)
            | ExprKind::GlobalRead(_)
            | ExprKind::Capture(_)
            | ExprKind::NoneLiteral => {}
            ExprKind::TupleLiteral(elements) | ExprKind::ArrayLiteral(elements) => {
                if matches!(&expr.kind, ExprKind::ArrayLiteral(_)) {
                    out.push((
                        expr.span,
                        "array allocation is not allowed in `@NoGC` code".to_string(),
                    ));
                }
                for element in elements {
                    self.collect_no_gc_expr_violations(element, out, requirements);
                }
            }
            ExprKind::StructInit { args, .. } | ExprKind::VariantConstruct { args, .. } => {
                for arg in args {
                    self.collect_no_gc_expr_violations(arg, out, requirements);
                }
            }
            ExprKind::ClassInit { args, .. } => {
                out.push((
                    expr.span,
                    "class allocation is not allowed in `@NoGC` code".to_string(),
                ));
                for arg in args {
                    self.collect_no_gc_expr_violations(arg, out, requirements);
                }
            }
            ExprKind::Lambda(_)
            | ExprKind::AnonymousFunction(_)
            | ExprKind::CallableReference(_)
            | ExprKind::FunctionCoercion { .. } => out.push((
                expr.span,
                "managed function values are not allowed in `@NoGC` code".to_string(),
            )),
            ExprKind::FieldAccess { receiver, field } => {
                if matches!(field, hir::FieldRef::ClassField { .. }) {
                    out.push((
                        expr.span,
                        "managed field access is not allowed in `@NoGC` code".to_string(),
                    ));
                }
                self.collect_no_gc_expr_violations(receiver, out, requirements);
            }
            ExprKind::MethodCall {
                receiver,
                callee,
                args,
            } => {
                match callee {
                    hir::MethodCallee::Callable(callee) => {
                        self.check_no_gc_callee(*callee, expr.span, out)
                    }
                    hir::MethodCallee::Bound(bound) => {
                        let member = self.bound_callable_refs[*bound].member;
                        let function = self.interface_method_entities[member].function;
                        self.check_no_gc_function(function, expr.span, out);
                    }
                }
                self.collect_no_gc_expr_violations(receiver, out, requirements);
                for arg in args {
                    self.collect_no_gc_expr_violations(arg, out, requirements);
                }
            }
            ExprKind::Box(operand) | ExprKind::Unbox(operand) => {
                out.push((
                    expr.span,
                    "boxing and unboxing are not allowed in `@NoGC` code".to_string(),
                ));
                self.collect_no_gc_expr_violations(operand, out, requirements);
            }
            ExprKind::IsInstance { operand, .. } | ExprKind::Cast { operand, .. } => {
                out.push((
                    expr.span,
                    "runtime type checks are not allowed in `@NoGC` code".to_string(),
                ));
                self.collect_no_gc_expr_violations(operand, out, requirements);
            }
            ExprKind::Index { receiver, index } => {
                out.push((
                    expr.span,
                    "array indexing is not allowed in `@NoGC` code".to_string(),
                ));
                self.collect_no_gc_expr_violations(receiver, out, requirements);
                self.collect_no_gc_expr_violations(index, out, requirements);
            }
            ExprKind::ArrayLen(operand) | ExprKind::ArrayClone(operand) => {
                out.push((
                    expr.span,
                    "array operations are not allowed in `@NoGC` code".to_string(),
                ));
                self.collect_no_gc_expr_violations(operand, out, requirements);
            }
            ExprKind::Call { callee, args } => {
                self.check_no_gc_callee(*callee, expr.span, out);
                for arg in args {
                    self.collect_no_gc_expr_violations(arg, out, requirements);
                }
            }
            ExprKind::LocalFunctionCall {
                callee,
                captures,
                args,
                ..
            } => {
                self.check_no_gc_callee(*callee, expr.span, out);
                for value in captures.iter().chain(args) {
                    self.collect_no_gc_expr_violations(value, out, requirements);
                }
            }
            ExprKind::CallableCall { callee, args, .. } => {
                out.push((
                    expr.span,
                    "managed function-value calls are not allowed in `@NoGC` code".to_string(),
                ));
                self.collect_no_gc_expr_violations(callee, out, requirements);
                for arg in args {
                    self.collect_no_gc_expr_violations(arg, out, requirements);
                }
            }
            ExprKind::ForeignCallbackRegister { closure, .. } => {
                out.push((
                    expr.span,
                    "managed callback registration is not allowed in `@NoGC` code".to_string(),
                ));
                self.collect_no_gc_expr_violations(closure, out, requirements);
            }
            ExprKind::ForeignCallbackOperation { callback, .. } => {
                out.push((
                    expr.span,
                    "managed callback token operations are not allowed in `@NoGC` code".to_string(),
                ));
                self.collect_no_gc_expr_violations(callback, out, requirements);
            }
            ExprKind::Binary { op, lhs, rhs } => {
                if *op == hir::BinOp::Div {
                    out.push((
                        expr.span,
                        "integer division is not allowed in `@NoGC` code because it may throw"
                            .to_string(),
                    ));
                }
                self.collect_no_gc_expr_violations(lhs, out, requirements);
                self.collect_no_gc_expr_violations(rhs, out, requirements);
            }
            ExprKind::Unary { operand, .. }
            | ExprKind::SomeWrap(operand)
            | ExprKind::IsSome(operand)
            | ExprKind::PtrFromUInt(operand)
            | ExprKind::PtrToUInt(operand)
            | ExprKind::PtrCast(operand) => {
                self.collect_no_gc_expr_violations(operand, out, requirements)
            }
            ExprKind::PtrLoad { pointer, offset } => {
                self.collect_no_gc_expr_violations(pointer, out, requirements);
                if let Some(offset) = offset {
                    self.collect_no_gc_expr_violations(offset, out, requirements);
                }
            }
            ExprKind::PtrOffset {
                pointer, offset, ..
            } => {
                self.collect_no_gc_expr_violations(pointer, out, requirements);
                self.collect_no_gc_expr_violations(offset, out, requirements);
            }
            ExprKind::PtrStore {
                pointer,
                offset,
                value,
            } => {
                self.collect_no_gc_expr_violations(pointer, out, requirements);
                if let Some(offset) = offset {
                    self.collect_no_gc_expr_violations(offset, out, requirements);
                }
                self.collect_no_gc_expr_violations(value, out, requirements);
            }
            // `addressOf` only materializes an already validated GC-free
            // place. It is unsafe, but does not allocate or enter the GC.
            ExprKind::AddressOf(_) => {}
            ExprKind::SizeOf(_)
            | ExprKind::AlignOf(_)
            | ExprKind::FunPtrNull
            | ExprKind::FunctionAddress(_) => {}
            ExprKind::Unwrap {
                operand,
                trap_on_none,
            } => {
                if *trap_on_none {
                    out.push((
                        expr.span,
                        "`!!` is not allowed in `@NoGC` code because it may throw".to_string(),
                    ));
                }
                self.collect_no_gc_expr_violations(operand, out, requirements);
            }
        }
    }

    fn check_no_gc_callee(
        &self,
        callable: hir::Callable,
        span: Span,
        out: &mut Vec<(Span, String)>,
    ) {
        let function = self.callable_function_id(callable);
        self.check_no_gc_function(function, span, out);
    }

    fn check_no_gc_function(
        &self,
        function: hir::FunctionId,
        span: Span,
        out: &mut Vec<(Span, String)>,
    ) {
        let callee = &self.functions[function];
        if callee.attributes.gc_effect != hir::GcEffect::NoGc {
            out.push((
                span,
                format!(
                    "`@NoGC` code may not call managed function `{}`",
                    callee.name
                ),
            ));
        }
    }
}
