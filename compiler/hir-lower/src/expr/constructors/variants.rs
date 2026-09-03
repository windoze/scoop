use super::*;

impl Lowerer {
    /// A unit variant construction (`None`, `Color.Red`): the variant
    /// carries no fields, so the enum's type arguments (if any) must
    /// come from the expected-type hint — the M3 `None` inference
    /// rule, generalized.
    pub(in crate::expr) fn lower_unit_variant(
        &mut self,
        name: &ast::Ident,
        enum_id: hir::EnumId,
        variant: u32,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
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
            crate::call_resolution::candidates::NominalConstructorSource::Variant {
                enumeration: enum_id,
                variant,
            },
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
                self.diagnose_nominal_failure(&view, &[], failure, name.span);
                return None;
            }
        };
        let application = self.enum_application_id(enum_id, type_args);
        let ty = self.enum_applications[application].canonical_type;
        Some(hir::Expr {
            kind: ExprKind::VariantConstruct {
                application,
                variant,
                args: Vec::new(),
            },
            ty,
            span: name.span,
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
        enum_id: hir::EnumId,
        variant: u32,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let CallSite {
            type_args: type_arg_refs,
            args,
            span,
        } = call;
        let view = self.nominal_constructor_view(
            crate::call_resolution::candidates::NominalConstructorSource::Variant {
                enumeration: enum_id,
                variant,
            },
        );
        let total = view.value_parameters.len();
        let supplied = args.len();
        if supplied > total {
            self.diagnose_nominal_shape_failure(
                &view,
                span,
                format!("expects {total} argument(s), but {supplied} were supplied"),
            );
            return None;
        }
        // Missing trailing fields must have constructor-style defaults.
        for index in supplied..total {
            if self.enums[enum_id].variants[variant as usize].defaults[index].is_none() {
                self.diagnose_nominal_shape_failure(
                    &view,
                    span,
                    format!("expects {total} argument(s), but {supplied} were supplied"),
                );
                return None;
            }
        }

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
        let argument_map = crate::call_resolution::arguments::CandidateArgumentMap::positional(
            supplied,
            crate::call_resolution::arguments::ReceiverInput::Absent,
        );
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
        let mut lowered = inferred.args;

        // Argument types must match the instantiated field types.
        for (field, arg) in view.value_parameters.iter().zip(&mut lowered) {
            let expected = self.instantiate_ty(field.ty, &type_args);
            debug_assert!(self.is_subtype(arg.ty, expected));
            *arg = self.adapt_to(arg.clone(), expected);
        }

        // Fill the trailing defaults (already lowered and type-checked
        // at the declaration site).
        for index in supplied..total {
            let default = self.enums[enum_id].variants[variant as usize].defaults[index]
                .as_ref()
                .expect("missing defaults were rejected above");
            lowered.push(clone_literal(default));
        }

        let application = self.enum_application_id(enum_id, type_args);
        let ty = self.enum_applications[application].canonical_type;
        Some(hir::Expr {
            kind: ExprKind::VariantConstruct {
                application,
                variant,
                args: lowered,
            },
            ty,
            span,
        })
    }
}
