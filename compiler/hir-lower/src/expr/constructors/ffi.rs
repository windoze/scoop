//! Compiler-known construction of raw FFI pointer values.

use super::*;

impl Lowerer {
    pub(in crate::expr) fn lower_ffi_struct_init(
        &mut self,
        struct_id: hir::StructId,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let mut view = self.nominal_constructor_view(
            crate::call_resolution::candidates::NominalConstructorSource::Struct(struct_id),
        );
        if Some(struct_id) == self.ffi_ptr {
            let argument_map =
                match crate::call_resolution::arguments::CandidateArgumentMap::source_nominal(
                    &view, call.args,
                ) {
                    Ok(argument_map) => argument_map,
                    Err(failure) => {
                        self.diagnose_nominal_shape_failure(&view, call.span, failure.describe());
                        return None;
                    }
                };
            let explicit = self.resolve_call_type_args(call.type_args)?;
            if !explicit.is_empty() && explicit.len() != view.owner_parameters.len() {
                self.diagnose_nominal_shape_failure(
                    &view,
                    call.span,
                    format!(
                        "expects {} explicit type argument(s), but {} were supplied",
                        view.owner_parameters.len(),
                        explicit.len()
                    ),
                );
                return None;
            }
            let expected_arguments = expected.and_then(|ty| match self.types[ty] {
                Type::Ptr(pointee) => Some(vec![pointee]),
                _ => None,
            });
            let inferred = self.lower_nominal_arguments(NominalArgumentInput {
                view: &view,
                argument_map: &argument_map,
                expressions: call.args,
                explicit_type_args: &explicit,
                expected_type_args: expected_arguments.as_deref(),
                span: call.span,
            })?;
            let [pointee] = inferred.type_args.as_slice() else {
                unreachable!("validated Ptr has one concrete pointee type")
            };
            let pointee = *pointee;
            if !self.is_value_ty(pointee)
                || self.type_contains_param(pointee)
                || !self.is_gc_free(pointee)
            {
                let found = self.type_name(pointee);
                self.diagnose_nominal_shape_failure(
                    &view,
                    call.span,
                    format!("`Ptr` pointee must be a concrete GC-free value type, found {found}"),
                );
                return None;
            }
            self.require_unsafe_operation(call.span, "constructing `Ptr` from a raw integer");
            let arguments = if inferred.argument_sinks.iter().all(Vec::is_empty) {
                inferred.args
            } else {
                self.materialize_nominal_arguments(
                    crate::argument_materialization::NominalArgumentMaterialization {
                        view: &view,
                        argument_map: &argument_map,
                        type_args: &inferred.type_args,
                        source_args: inferred.args,
                        argument_sinks: inferred.argument_sinks,
                        call_span: call.span,
                    },
                    sink,
                )
            };
            let [raw]: [hir::Expr; 1] = arguments
                .try_into()
                .expect("validated Ptr constructor has one argument");
            debug_assert!(self.is_subtype(raw.ty, self.uint));
            let raw = self.adapt_to(raw, self.uint);
            let ty = self.intern_type(Type::Ptr(pointee));
            return Some(hir::Expr {
                kind: ExprKind::PtrFromUInt(Box::new(raw)),
                ty,
                span: call.span,
            });
        }

        debug_assert_eq!(Some(struct_id), self.ffi_fun_ptr);
        view.value_parameters.clear();
        let argument_map =
            match crate::call_resolution::arguments::CandidateArgumentMap::source_nominal(
                &view, call.args,
            ) {
                Ok(argument_map) => argument_map,
                Err(failure) => {
                    self.diagnose_nominal_shape_failure(&view, call.span, failure.describe());
                    return None;
                }
            };
        let explicit = self.resolve_call_type_args(call.type_args)?;
        if !explicit.is_empty() && explicit.len() != view.owner_parameters.len() {
            self.diagnose_nominal_shape_failure(
                &view,
                call.span,
                format!(
                    "expects {} explicit type argument(s), but {} were supplied",
                    view.owner_parameters.len(),
                    explicit.len()
                ),
            );
            return None;
        }
        let expected_arguments = expected.and_then(|ty| match self.types[ty] {
            Type::FunPtr(signature) => Some(vec![self.function_types[signature].canonical_type]),
            _ => None,
        });
        let inferred = self.lower_nominal_arguments(NominalArgumentInput {
            view: &view,
            argument_map: &argument_map,
            expressions: call.args,
            explicit_type_args: &explicit,
            expected_type_args: expected_arguments.as_deref(),
            span: call.span,
        })?;
        let [function] = inferred.type_args.as_slice() else {
            unreachable!("validated FunPtr has one concrete function type")
        };
        let Type::Function(signature) = self.types[*function] else {
            unreachable!("FunPtr concrete application accepts only function types")
        };
        let ty = self.intern_type(Type::FunPtr(signature));
        Some(hir::Expr {
            kind: ExprKind::FunPtrNull,
            ty,
            span: call.span,
        })
    }
}
