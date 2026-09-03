use super::*;

impl Lowerer {
    /// Struct construction with positional arguments: argument count
    /// and types must match the declared fields one by one (the field
    /// type is the argument's expected-type hint).
    pub(in crate::expr) fn lower_struct_init(
        &mut self,
        struct_id: hir::StructId,
        definition_ty: TypeId,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let CallSite {
            type_args: type_arg_refs,
            args,
            span,
        } = call;
        let name = self.structs[struct_id].name.clone();
        if matches!(
            self.structs[struct_id].representation,
            hir::StructRepresentation::Intrinsic(_)
        ) {
            self.error(
                span,
                format!("intrinsic struct `{name}` has no source constructor"),
            );
            return None;
        }
        let view = self.nominal_constructor_view(
            crate::call_resolution::candidates::NominalConstructorSource::Struct(struct_id),
        );
        let argument_map =
            match crate::call_resolution::arguments::CandidateArgumentMap::exact_nominal(
                &view,
                args.len(),
            ) {
                Ok(argument_map) => argument_map,
                Err(mismatch) => {
                    self.diagnose_nominal_shape_failure(
                        &view,
                        span,
                        format!(
                            "expects {} argument(s), but {} were supplied",
                            mismatch.expected, mismatch.supplied
                        ),
                    );
                    return None;
                }
            };
        let explicit_type_args = self.resolve_call_type_args(type_arg_refs)?;
        let type_param_count = view.owner_parameters.len();
        if !explicit_type_args.is_empty() && explicit_type_args.len() != type_param_count {
            self.diagnose_nominal_shape_failure(
                &view,
                span,
                format!(
                    "expects {type_param_count} explicit type argument(s), but {} were supplied",
                    explicit_type_args.len()
                ),
            );
            return None;
        }
        let expected_arguments = expected.and_then(|expected| {
            let Type::Struct(application) = self.types[expected] else {
                return None;
            };
            let application = &self.struct_applications[application];
            (application.template == struct_id && application.arguments.len() == type_param_count)
                .then(|| application.arguments.clone())
        });
        let inferred = self.lower_nominal_arguments(
            NominalArgumentInput {
                view: &view,
                argument_map: &argument_map,
                expressions: args,
                explicit_type_args: &explicit_type_args,
                expected_type_args: expected_arguments.as_deref(),
                span,
            },
            sink,
        )?;
        let type_args = inferred.type_args;
        let lowered = inferred.args;

        let mut adapted = Vec::with_capacity(lowered.len());
        for (field, arg) in view.value_parameters.iter().zip(lowered) {
            let field_ty = self.instantiate_ty(field.ty, &type_args);
            debug_assert!(self.is_subtype(arg.ty, field_ty));
            adapted.push(self.adapt_to(arg, field_ty));
        }
        let application = self.struct_application_id(struct_id, type_args);
        let ty = self.struct_applications[application].canonical_type;
        debug_assert!(
            !self.structs[struct_id].type_params.is_empty() || self.types_equal(ty, definition_ty)
        );
        Some(hir::Expr {
            kind: ExprKind::StructInit {
                application,
                args: adapted,
            },
            ty,
            span,
        })
    }
}
