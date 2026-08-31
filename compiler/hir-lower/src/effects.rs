//! Verification of M12 safety and `@NoGC` effects over resolved HIR.

use std::collections::HashSet;

use scoop_ast::Span;
use scoop_hir as hir;

use crate::Lowerer;

impl Lowerer {
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
                if !self.is_gc_free(param.ty) {
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
            if !self.is_gc_free(function.return_ty) {
                self.error(
                    function.span,
                    format!(
                        "`@NoGC` function `{}` has non-GC-free return type {}",
                        function.name,
                        self.type_name(function.return_ty)
                    ),
                );
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
                if !self.is_gc_free(local.ty) {
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
            let mut violations = Vec::new();
            self.collect_no_gc_statement_violations(&body.statements, &mut violations);
            for (span, message) in violations {
                self.error(span, message);
            }
        }

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
        self.is_gc_free_inner(ty, &[], &mut HashSet::new())
    }

    fn is_gc_free_inner(
        &self,
        ty: hir::TypeId,
        substitution: &[hir::TypeId],
        visiting: &mut HashSet<hir::TypeId>,
    ) -> bool {
        match &self.types[ty] {
            hir::Type::Unit
            | hir::Type::Int
            | hir::Type::UInt
            | hir::Type::Boolean
            | hir::Type::Ptr(_)
            | hir::Type::FunPtr(_) => true,
            hir::Type::String
            | hir::Type::Class(_)
            | hir::Type::Interface(_, _)
            | hir::Type::Any
            | hir::Type::Array(_)
            | hir::Type::MutableArray(_)
            | hir::Type::Function(_) => false,
            hir::Type::Tuple(elements) => elements
                .iter()
                .all(|ty| self.is_gc_free_inner(*ty, substitution, visiting)),
            hir::Type::Param(index) => substitution
                .get(index.into_raw() as usize)
                .is_some_and(|ty| self.is_gc_free_inner(*ty, &[], visiting)),
            hir::Type::Struct(id, args) => {
                if !visiting.insert(ty) {
                    return false;
                }
                let result = self.structs[*id]
                    .fields
                    .iter()
                    .all(|field| self.is_gc_free_inner(field.ty, args, visiting));
                visiting.remove(&ty);
                result
            }
            hir::Type::Enum(id, args) => {
                if !visiting.insert(ty) {
                    return false;
                }
                let result = self.enums[*id].variants.iter().all(|variant| {
                    variant
                        .fields
                        .iter()
                        .all(|field| self.is_gc_free_inner(field.ty, args, visiting))
                });
                visiting.remove(&ty);
                result
            }
        }
    }

    pub(crate) fn requires_unsafe_use(&self, ty: hir::TypeId) -> bool {
        self.requires_unsafe_use_inner(ty, &[], &mut HashSet::new())
    }

    fn requires_unsafe_use_inner(
        &self,
        ty: hir::TypeId,
        substitution: &[hir::TypeId],
        visiting: &mut HashSet<hir::TypeId>,
    ) -> bool {
        match &self.types[ty] {
            hir::Type::Struct(id, args) => {
                if self.structs[*id].attributes.interior_mutable {
                    return true;
                }
                if !visiting.insert(ty) {
                    return false;
                }
                let result = self.structs[*id]
                    .fields
                    .iter()
                    .any(|field| self.requires_unsafe_use_inner(field.ty, args, visiting));
                visiting.remove(&ty);
                result
            }
            hir::Type::Enum(id, args) => {
                if !visiting.insert(ty) {
                    return false;
                }
                let result = self.enums[*id].variants.iter().any(|variant| {
                    variant
                        .fields
                        .iter()
                        .any(|field| self.requires_unsafe_use_inner(field.ty, args, visiting))
                });
                visiting.remove(&ty);
                result
            }
            hir::Type::Tuple(elements) => elements
                .iter()
                .any(|ty| self.requires_unsafe_use_inner(*ty, substitution, visiting)),
            hir::Type::Param(index) => substitution
                .get(index.into_raw() as usize)
                .is_some_and(|ty| self.requires_unsafe_use_inner(*ty, &[], visiting)),
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
    ) {
        for statement in statements {
            match &statement.kind {
                hir::StatementKind::Expr(expr) => self.collect_no_gc_expr_violations(expr, out),
                hir::StatementKind::LocalFunction(_) => {}
                hir::StatementKind::Return { value } => {
                    if let Some(value) = value {
                        self.collect_no_gc_expr_violations(value, out);
                    }
                }
                hir::StatementKind::ValDecl { init, .. } => {
                    self.collect_no_gc_expr_violations(init, out)
                }
                hir::StatementKind::Assign { target, value } => {
                    match target {
                        hir::AssignTarget::Local(_) => {}
                        hir::AssignTarget::Index { array, index } => {
                            out.push((
                                statement.span,
                                "array assignment is not allowed in `@NoGC` code".to_string(),
                            ));
                            self.collect_no_gc_expr_violations(array, out);
                            self.collect_no_gc_expr_violations(index, out);
                        }
                        hir::AssignTarget::Field { receiver, .. } => {
                            out.push((
                                statement.span,
                                "managed field assignment is not allowed in `@NoGC` code"
                                    .to_string(),
                            ));
                            self.collect_no_gc_expr_violations(receiver, out);
                        }
                    }
                    self.collect_no_gc_expr_violations(value, out);
                }
                hir::StatementKind::If {
                    cond,
                    then_body,
                    else_body,
                } => {
                    self.collect_no_gc_expr_violations(cond, out);
                    self.collect_no_gc_statement_violations(then_body, out);
                    if let Some(else_body) = else_body {
                        self.collect_no_gc_statement_violations(else_body, out);
                    }
                }
                hir::StatementKind::While { cond, body } => {
                    self.collect_no_gc_expr_violations(cond, out);
                    self.collect_no_gc_statement_violations(body, out);
                }
                hir::StatementKind::When(when) => {
                    self.collect_no_gc_expr_violations(&when.subject, out);
                    for arm in &when.arms {
                        if let Some(guard) = &arm.guard {
                            self.collect_no_gc_expr_violations(guard, out);
                        }
                        self.collect_no_gc_statement_violations(&arm.body, out);
                    }
                    if let Some(else_body) = &when.else_body {
                        self.collect_no_gc_statement_violations(else_body, out);
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
                    self.collect_no_gc_expr_violations(expr, out);
                }
            }
        }
    }

    fn collect_no_gc_expr_violations(&self, expr: &hir::Expr, out: &mut Vec<(Span, String)>) {
        use hir::ExprKind;
        if !self.is_gc_free(expr.ty) {
            out.push((
                expr.span,
                format!(
                    "value of non-GC-free type {} is not allowed in `@NoGC` code",
                    self.type_name(expr.ty)
                ),
            ));
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
                    self.collect_no_gc_expr_violations(element, out);
                }
            }
            ExprKind::StructInit { args, .. } | ExprKind::VariantConstruct { args, .. } => {
                for arg in args {
                    self.collect_no_gc_expr_violations(arg, out);
                }
            }
            ExprKind::ClassInit { args, .. } => {
                out.push((
                    expr.span,
                    "class allocation is not allowed in `@NoGC` code".to_string(),
                ));
                for arg in args {
                    self.collect_no_gc_expr_violations(arg, out);
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
                self.collect_no_gc_expr_violations(receiver, out);
            }
            ExprKind::MethodCall {
                receiver,
                callee,
                args,
            } => {
                self.check_no_gc_callee(*callee, expr.span, out);
                self.collect_no_gc_expr_violations(receiver, out);
                for arg in args {
                    self.collect_no_gc_expr_violations(arg, out);
                }
            }
            ExprKind::Box(operand) | ExprKind::Unbox(operand) => {
                out.push((
                    expr.span,
                    "boxing and unboxing are not allowed in `@NoGC` code".to_string(),
                ));
                self.collect_no_gc_expr_violations(operand, out);
            }
            ExprKind::IsInstance { operand, .. } | ExprKind::Cast { operand, .. } => {
                out.push((
                    expr.span,
                    "runtime type checks are not allowed in `@NoGC` code".to_string(),
                ));
                self.collect_no_gc_expr_violations(operand, out);
            }
            ExprKind::Index { receiver, index } => {
                out.push((
                    expr.span,
                    "array indexing is not allowed in `@NoGC` code".to_string(),
                ));
                self.collect_no_gc_expr_violations(receiver, out);
                self.collect_no_gc_expr_violations(index, out);
            }
            ExprKind::ArrayLen(operand) | ExprKind::ArrayClone(operand) => {
                out.push((
                    expr.span,
                    "array operations are not allowed in `@NoGC` code".to_string(),
                ));
                self.collect_no_gc_expr_violations(operand, out);
            }
            ExprKind::Call { callee, args } => {
                self.check_no_gc_callee(*callee, expr.span, out);
                for arg in args {
                    self.collect_no_gc_expr_violations(arg, out);
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
                    self.collect_no_gc_expr_violations(value, out);
                }
            }
            ExprKind::CallableCall { callee, args, .. } => {
                out.push((
                    expr.span,
                    "managed function-value calls are not allowed in `@NoGC` code".to_string(),
                ));
                self.collect_no_gc_expr_violations(callee, out);
                for arg in args {
                    self.collect_no_gc_expr_violations(arg, out);
                }
            }
            ExprKind::Binary { op, lhs, rhs } => {
                if *op == hir::BinOp::Div {
                    out.push((
                        expr.span,
                        "integer division is not allowed in `@NoGC` code because it may throw"
                            .to_string(),
                    ));
                }
                self.collect_no_gc_expr_violations(lhs, out);
                self.collect_no_gc_expr_violations(rhs, out);
            }
            ExprKind::Unary { operand, .. }
            | ExprKind::SomeWrap(operand)
            | ExprKind::IsSome(operand)
            | ExprKind::PtrFromUInt(operand)
            | ExprKind::PtrToUInt(operand)
            | ExprKind::PtrCast(operand) => self.collect_no_gc_expr_violations(operand, out),
            ExprKind::PtrLoad { pointer, offset } => {
                self.collect_no_gc_expr_violations(pointer, out);
                if let Some(offset) = offset {
                    self.collect_no_gc_expr_violations(offset, out);
                }
            }
            ExprKind::PtrOffset {
                pointer, offset, ..
            } => {
                self.collect_no_gc_expr_violations(pointer, out);
                self.collect_no_gc_expr_violations(offset, out);
            }
            ExprKind::PtrStore {
                pointer,
                offset,
                value,
            } => {
                self.collect_no_gc_expr_violations(pointer, out);
                if let Some(offset) = offset {
                    self.collect_no_gc_expr_violations(offset, out);
                }
                self.collect_no_gc_expr_violations(value, out);
            }
            ExprKind::AddressOf(_) => out.push((
                expr.span,
                "`addressOf` is not allowed in `@NoGC` code".to_string(),
            )),
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
                self.collect_no_gc_expr_violations(operand, out);
            }
        }
    }

    fn check_no_gc_callee(
        &self,
        callable: hir::Callable,
        span: Span,
        out: &mut Vec<(Span, String)>,
    ) {
        let function = match callable {
            hir::Callable::Function(function) => function,
            hir::Callable::Generic(instantiation) => {
                let generic = self.instantiations[instantiation].generic;
                self.generic_functions[generic].function
            }
        };
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
