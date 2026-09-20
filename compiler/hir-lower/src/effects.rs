//! Verification of M12 safety and `@NoGC` effects over resolved HIR.

use std::collections::HashSet;

use scoop_ast::Span;
use scoop_hir as hir;

use crate::Lowerer;

mod constructors;
mod gc_free_pointees;
mod generic_recursion;
mod no_gc_generics;
mod type_properties;
mod type_validation;
mod violations;

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
                .unwrap_or_else(|| self.primary_output_file());
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

        self.check_no_gc_constructors();
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
                .unwrap_or_else(|| self.primary_output_file());
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
}

impl Lowerer {
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
