use super::*;
use scoop_identity::CallbackMode;

impl Lowerer {
    pub(in crate::expr) fn normalize_foreign_callback_registration(
        &mut self,
        native_ty: hir::TypeId,
        arguments: Vec<hir::Expr>,
        result_type: hir::TypeId,
        span: Span,
        sink: &[hir::Statement],
    ) -> Option<hir::Expr> {
        let hir::Type::Function(native_function_type) = self.types[native_ty] else {
            unreachable!("callback inference checked its explicit function type")
        };
        let native = self.function_types[native_function_type].clone();
        let [closure, context, mode]: [hir::Expr; 3] = arguments
            .try_into()
            .expect("a registration has three arguments");
        let ExprKind::IntegerLiteral(hir::HirIntegerConstant::Signed64(index)) =
            materialized_source(&context, sink).kind
        else {
            unreachable!("callback inference checked its literal context index")
        };
        let context_index = index as usize;
        let parameters = native
            .parameter_types
            .iter()
            .enumerate()
            .filter_map(|(index, ty)| (index != context_index).then_some(*ty))
            .collect();
        let managed_ty = self.intern_function_type(false, parameters, native.return_type);
        let hir::Type::Function(managed_function_type) = self.types[managed_ty] else {
            unreachable!("the managed callback signature is a function type")
        };
        if !self.types_equal(closure.ty, managed_ty) {
            self.error(
                closure.span,
                format!(
                    "foreign callback closure must have type {}, found {}",
                    self.type_name(managed_ty),
                    self.type_name(closure.ty)
                ),
            );
            return None;
        }
        let declared_mode = match &materialized_source(&mode, sink).kind {
            ExprKind::VariantConstruct { variant, args } if args.is_empty() => {
                self.callback_mode(*variant)
            }
            _ => None,
        };
        let Some(mode_value) = declared_mode else {
            self.error(
                mode.span,
                "foreign callback mode must be the constant `Reusable` or `OneShot`".into(),
            );
            return None;
        };
        let definition_path = self
            .definition_paths
            .next(scoop_identity::StructuralDefinitionSiteRole::CallbackConversion);
        let registration =
            self.foreign_callback_registrations
                .alloc(hir::ForeignCallbackRegistration {
                    definition: hir::ForeignCallbackDefinition::Source {
                        root: self.current_definition_root(),
                        path: definition_path,
                    },
                    native_function_type,
                    managed_function_type,
                    context_index: context_index as u32,
                    mode: mode_value,
                    span,
                });
        Some(hir::Expr {
            kind: ExprKind::ForeignCallbackRegister {
                registration,
                closure: Box::new(closure),
            },
            ty: result_type,
            span,
            origin: self.expression_origin(span),
        })
    }

    fn callback_mode(&self, variant: hir::EnumVariantApplication) -> Option<CallbackMode> {
        let (reusable, one_shot) = match &self.core {
            crate::CoreLoweringAuthority::Defined => {
                let core = self
                    .foreign_callback_core
                    .expect("defined core callback roles are complete");
                (
                    self.enum_variant_reference(core.modes.reusable()).variant,
                    self.enum_variant_reference(core.modes.one_shot()).variant,
                )
            }
            crate::CoreLoweringAuthority::Imported(core) => (
                core.foreign_callbacks().reusable().persistent(),
                core.foreign_callbacks().one_shot().persistent(),
            ),
        };
        if variant.variant == reusable {
            Some(CallbackMode::Reusable)
        } else if variant.variant == one_shot {
            Some(CallbackMode::OneShot)
        } else {
            None
        }
    }
}
