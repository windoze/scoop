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
            match crate::call_resolution::arguments::CandidateArgumentMap::source_nominal(
                &view, args,
            ) {
                Ok(argument_map) => argument_map,
                Err(failure) => {
                    self.diagnose_nominal_shape_failure(&view, span, failure.describe());
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
        let inferred = self.lower_nominal_arguments(NominalArgumentInput {
            view: &view,
            argument_map: &argument_map,
            expressions: args,
            explicit_type_args: &explicit_type_args,
            expected_type_args: expected_arguments.as_deref(),
            span,
        })?;
        let type_args = inferred.type_args;
        let adapted = if argument_map.is_identity_explicit()
            && inferred.argument_sinks.iter().all(Vec::is_empty)
        {
            view.value_parameters
                .iter()
                .zip(inferred.args)
                .map(|(parameter, argument)| {
                    let expected = self.instantiate_ty(parameter.ty, &type_args);
                    debug_assert!(self.is_subtype(argument.ty, expected));
                    self.adapt_to(argument, expected)
                })
                .collect()
        } else {
            self.materialize_nominal_arguments(
                crate::argument_materialization::NominalArgumentMaterialization {
                    view: &view,
                    argument_map: &argument_map,
                    type_args: &type_args,
                    source_args: inferred.args,
                    argument_sinks: inferred.argument_sinks,
                    call_span: span,
                },
                sink,
            )
        };
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
            origin: self.expression_origin(span),
        })
    }
}
