use super::*;

impl Lowerer {
    /// A unit variant construction (`Color.Red`): the variant carries no
    /// fields, so the enum's type arguments (if any) must come from the
    /// expected-type hint.
    pub(in crate::expr) fn lower_unit_variant(
        &mut self,
        name: &ast::Ident,
        target: hir::EnumVariantRef,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        debug_assert!(self.resolved_variant_style(target) == VariantStyle::Unit);
        let enum_id = target.enumeration();
        let arity = self.enums[enum_id].type_params.len();
        let expected_arguments = expected.and_then(|ty| match self.types[ty].clone() {
            Type::Enum(application) => {
                let application = &self.enum_applications[application];
                (application.template == enum_id && application.arguments.len() == arity)
                    .then(|| application.arguments.clone())
            }
            _ => None,
        });
        let view = self.nominal_constructor_view(
            crate::call_resolution::candidates::NominalConstructorSource::Variant(target),
        );
        let argument_map = crate::call_resolution::arguments::CandidateArgumentMap::positional(
            0,
            crate::call_resolution::arguments::ReceiverInput::Absent,
        );
        let type_args = match self.solve_nominal_applicability(
            crate::call_resolution::applicability::NominalApplicabilityInput {
                view: &view,
                argument_map: &argument_map,
                explicit_arguments: &[],
                expected_arguments: expected_arguments.as_deref(),
                argument_types: &[],
            },
        ) {
            Ok(type_args) => type_args,
            Err(failure) => {
                self.diagnose_nominal_failure(&view, &argument_map, &[], &[], failure, name.span);
                return None;
            }
        };
        let application = self.enum_application_id(enum_id, type_args);
        let variant = hir::AppliedEnumVariantRef::checked(
            &self.enums,
            &self.enum_applications,
            application,
            target,
        )
        .expect("the selected application belongs to the checked variant declaration");
        let ty = self.enum_applications[application].canonical_type;
        Some(hir::Expr {
            kind: ExprKind::VariantConstruct {
                variant,
                args: Vec::new(),
            },
            ty,
            span: name.span,
            origin: self.expression_origin(name.span),
        })
    }

    /// Variant construction (`Some(x)`, `Shape.Circle(1)`,
    /// `E.WithDefault(1)` with a trailing default filled in). The
    /// variant behaves like a generic constructor function: type
    /// arguments are seeded from an expected `E<...>` hint and then
    /// inferred from the arguments (the same binding mechanism as
    /// generic calls), and each argument is checked against the
    /// instantiated field type.
    pub(in crate::expr) fn lower_variant_construct(
        &mut self,
        target: hir::EnumVariantRef,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        if self.resolved_variant_style(target) == VariantStyle::Unit {
            let enumeration = target.enumeration();
            let variant = &self.enums[enumeration].variants[target.local_index() as usize];
            self.error(
                call.span,
                format!(
                    "unit variant `{}` of `{}` does not take arguments; use `{}` without parentheses",
                    variant.name, self.enums[enumeration].name, variant.name
                ),
            );
            return None;
        }
        let enum_id = target.enumeration();
        let CallSite {
            type_args: type_arg_refs,
            args,
            span,
        } = call;
        let view = self.nominal_constructor_view(
            crate::call_resolution::candidates::NominalConstructorSource::Variant(target),
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
            let Type::Enum(application) = self.types[expected] else {
                return None;
            };
            let application = &self.enum_applications[application];
            (application.template == enum_id && application.arguments.len() == type_param_count)
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
        let lowered = self.materialize_nominal_arguments(
            crate::argument_materialization::NominalArgumentMaterialization {
                view: &view,
                argument_map: &argument_map,
                type_args: &type_args,
                source_args: inferred.args,
                argument_sinks: inferred.argument_sinks,
                call_span: span,
            },
            sink,
        );

        let application = self.enum_application_id(enum_id, type_args);
        let variant = hir::AppliedEnumVariantRef::checked(
            &self.enums,
            &self.enum_applications,
            application,
            target,
        )
        .expect("the selected application belongs to the checked variant declaration");
        let ty = self.enum_applications[application].canonical_type;
        Some(hir::Expr {
            kind: ExprKind::VariantConstruct {
                variant,
                args: lowered,
            },
            ty,
            span,
            origin: self.expression_origin(span),
        })
    }
}
