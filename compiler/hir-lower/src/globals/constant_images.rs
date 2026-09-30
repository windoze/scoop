//! Constant images for global and singleton storage.

use super::*;

impl Lowerer {
    pub(super) fn zero_constant_image(&mut self, ty: hir::TypeId) -> Option<hir::HirConstantImage> {
        match self.types[ty].clone() {
            hir::Type::Integer(kind) => Some(hir::HirConstantImage::Integer(
                hir::HirIntegerConstant::from_magnitude(kind, 0, false)
                    .expect("zero is representable by every integer kind"),
            )),
            hir::Type::Boolean => Some(hir::HirConstantImage::Boolean(false)),
            hir::Type::Enum(_) => self.static_none_constant(ty),
            hir::Type::Struct(application) => {
                let application_value = self.struct_applications[application].clone();
                let fields = self
                    .struct_definition(application_value.template)
                    .semantic_fields()
                    .to_vec();
                let fields = fields
                    .into_iter()
                    .map(|field| {
                        let ty = self.instantiate_ty(field.ty, &application_value.arguments);
                        self.zero_constant_image(ty)
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(hir::HirConstantImage::Struct {
                    application,
                    fields,
                })
            }
            hir::Type::Unit
            | hir::Type::String
            | hir::Type::Class(_)
            | hir::Type::Interface(_)
            | hir::Type::Any
            | hir::Type::Tuple(_)
            | hir::Type::Function(_)
            | hir::Type::Ptr(_)
            | hir::Type::FunPtr(_)
            | hir::Type::Param(_) => None,
        }
    }

    pub(super) fn static_none_constant(&self, ty: hir::TypeId) -> Option<hir::HirConstantImage> {
        self.as_option(ty)?;
        let variant = match &self.core {
            crate::CoreLoweringAuthority::Imported(protocols) => {
                protocols.option().none().persistent()
            }
            _ => self.enum_member_identities.as_ref()?[self.option_core?.none()].id(),
        };
        Some(hir::HirConstantImage::EnumUnit {
            variant: hir::EnumVariantApplication { owner: ty, variant },
        })
    }

    pub(super) fn static_unit_variant_constant(
        &mut self,
        expression: &ast::Expr,
        expected: hir::TypeId,
    ) -> Option<hir::HirConstantImage> {
        if !matches!(self.types[expected], hir::Type::Enum(_))
            || !matches!(expression, ast::Expr::Var(_) | ast::Expr::FieldAccess(_))
        {
            return None;
        }
        let mut probe = self.clone();
        let mut sink = Vec::new();
        let value = probe.lower_expr(expression, &mut sink, Some(expected))?;
        let hir::ExprKind::VariantConstruct { variant, args } = value.kind else {
            return None;
        };
        if !args.is_empty()
            || !sink.is_empty()
            || !probe.types_equal(value.ty, expected)
            || probe.diagnostics.len() != self.diagnostics.len()
        {
            return None;
        }
        self.dependencies = probe.dependencies;
        Some(hir::HirConstantImage::EnumUnit {
            variant: hir::EnumVariantApplication {
                owner: expected,
                variant: variant.variant,
            },
        })
    }

    pub(super) fn global_constant(
        &mut self,
        expr: &ast::Expr,
        expected: hir::TypeId,
    ) -> Option<hir::HirConstantImage> {
        if let Some(variant) = self.static_unit_variant_constant(expr, expected) {
            return Some(variant);
        }
        match (self.types[expected].clone(), expr) {
            (hir::Type::Integer(_), ast::Expr::IntLiteral(literal)) => self
                .lower_integer_literal(*literal, Some(expected), false, literal.span)
                .and_then(|value| match value.kind {
                    hir::ExprKind::IntegerLiteral(value) => {
                        Some(hir::HirConstantImage::Integer(value))
                    }
                    _ => None,
                }),
            (
                hir::Type::Integer(_),
                ast::Expr::Unary {
                    op: ast::UnOp::Neg,
                    operand,
                    span,
                },
            ) => match &**operand {
                ast::Expr::IntLiteral(literal)
                    if matches!(
                        literal.suffix,
                        ast::IntegerSuffix::None | ast::IntegerSuffix::Long
                    ) =>
                {
                    self.lower_integer_literal(*literal, Some(expected), true, *span)
                        .and_then(|value| match value.kind {
                            hir::ExprKind::IntegerLiteral(value) => {
                                Some(hir::HirConstantImage::Integer(value))
                            }
                            _ => None,
                        })
                }
                _ => None,
            },
            (hir::Type::Boolean, ast::Expr::BoolLiteral { value, .. }) => {
                Some(hir::HirConstantImage::Boolean(*value))
            }
            (hir::Type::Struct(application), ast::Expr::Call(call)) => {
                let application_value = self.struct_applications[application].clone();
                let struct_id = self.source_struct_id(application_value.template)?;
                if self.global_struct_callee(call) != Some(struct_id) {
                    return None;
                }
                let constructor = self.struct_primary_constructor(struct_id)?;
                let view = self.nominal_constructor_view(
                    NominalConstructorSource::Struct(constructor),
                    call.span,
                );
                let (values, _) =
                    self.global_nominal_constant(&view, call, &application_value.arguments)?;
                Some(hir::HirConstantImage::Struct {
                    application,
                    fields: values,
                })
            }
            _ => None,
        }
    }

    pub(super) fn global_struct_callee(&self, call: &ast::CallExpr) -> Option<hir::StructId> {
        self.top_level_struct_named(&call.callee.text)
            .map(|(structure, _)| structure)
    }

    pub(super) fn global_nominal_constant(
        &mut self,
        view: &NominalConstructorView,
        call: &ast::CallExpr,
        expected_arguments: &[hir::TypeId],
    ) -> Option<(Vec<hir::HirConstantImage>, Vec<hir::TypeId>)> {
        if expected_arguments.len() != view.signature.owner_parameters.len() {
            return None;
        }
        let argument_map = CandidateArgumentMap::source_nominal(view, &call.args).ok()?;
        let explicit_arguments = self.resolve_call_type_args(&call.type_args)?;
        if !explicit_arguments.is_empty()
            && explicit_arguments.len() != view.signature.owner_parameters.len()
        {
            return None;
        }
        let seed = if explicit_arguments.is_empty() {
            expected_arguments.to_vec()
        } else {
            explicit_arguments
                .iter()
                .zip(expected_arguments)
                .map(|(argument, &expected)| match argument {
                    crate::expr::ResolvedCallTypeArgument::Explicit { ty, .. } => *ty,
                    crate::expr::ResolvedCallTypeArgument::Infer { .. } => expected,
                })
                .collect()
        };
        let parameter_types = view
            .signature
            .value_parameters
            .iter()
            .map(|parameter| self.instantiate_ty(parameter.ty, &seed))
            .collect::<Vec<_>>();
        let values = call
            .args
            .iter()
            .zip(&parameter_types)
            .map(|(argument, &parameter)| self.global_constant(&argument.expression, parameter))
            .collect::<Option<Vec<_>>>()?;
        let argument_types = parameter_types
            .iter()
            .copied()
            .map(Some)
            .collect::<Vec<_>>();
        let solution = self
            .solve_nominal_applicability(NominalApplicabilityInput {
                view,
                argument_map: &argument_map,
                explicit_arguments: &explicit_arguments,
                expected_arguments: Some(expected_arguments),
                argument_types: &argument_types,
            })
            .ok()?;
        solution
            .iter()
            .zip(expected_arguments)
            .all(|(&actual, &expected)| self.types_equal(actual, expected))
            .then_some((values, solution))
    }
}
