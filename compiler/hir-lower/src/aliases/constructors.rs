use super::*;
use crate::call_resolution::candidates::{DeclarationSignature, ValueParameterCalling};
use crate::expr::ResolvedCallTypeArgument;

pub(crate) struct AliasConstructorSignature<D> {
    pub(crate) signature: DeclarationSignature<D>,
    pub(crate) explicit: Vec<ResolvedCallTypeArgument>,
    pub(crate) target_bindings: Vec<(hir::TypeParamId, hir::TypeId)>,
}

impl Lowerer {
    /// Instantiate the alias's binders, then project the real constructor
    /// through its fixed target application. No forwarding declaration is made.
    pub(crate) fn alias_constructor_signature<D: Clone>(
        &mut self,
        alias: &AliasTemplate,
        original: &DeclarationSignature<D>,
        explicit: &[ResolvedCallTypeArgument],
        span: ast::Span,
    ) -> Option<AliasConstructorSignature<D>> {
        let own_count = alias.parameters.len() - alias.outer_parameter_count;
        if !explicit.is_empty() && explicit.len() != own_count {
            self.error(span, format!("typealias constructor expects {own_count} type argument(s), but {} were supplied", explicit.len()));
            return None;
        }
        let mut arguments = self
            .alias_outer_arguments(alias, alias.host, span)?
            .into_iter()
            .map(|ty| ResolvedCallTypeArgument::Explicit { ty, span })
            .collect::<Vec<_>>();
        if explicit.is_empty() {
            arguments.extend((0..own_count).map(|_| ResolvedCallTypeArgument::Infer { span }));
        } else {
            arguments.extend_from_slice(explicit);
        }
        if let Some(fixed) = &alias.arguments {
            arguments = fixed
                .iter()
                .map(|ty| ResolvedCallTypeArgument::Explicit { ty: *ty, span })
                .collect();
        }
        let mut parameters = alias.parameters.clone();
        let bindings = parameters
            .iter_mut()
            .enumerate()
            .map(|(index, parameter)| {
                let original = parameter.id;
                parameter.id = self.fresh_type_param(index);
                (original, self.intern_type(Type::Param(parameter.id)))
            })
            .collect::<Vec<_>>();
        for parameter in &mut parameters {
            if let hir::TypeParamBounds::Nominal(bounds) = &mut parameter.bounds {
                if let Some(bound) = &mut bounds.class {
                    bound.ty = self.instantiate_method_ty(bound.ty, &bindings);
                }
                for bound in &mut bounds.interfaces {
                    bound.ty = self.instantiate_method_ty(bound.ty, &bindings);
                }
            }
        }
        let target = if let Some(arguments) = &alias.arguments {
            let bindings = alias
                .parameters
                .iter()
                .map(|parameter| parameter.id)
                .zip(arguments.iter().copied())
                .collect::<Vec<_>>();
            self.instantiate_method_ty(alias.target, &bindings)
        } else {
            self.instantiate_method_ty(alias.target, &bindings)
        };
        let target_arguments = match self.types[target] {
            Type::Ptr(pointee) => vec![pointee],
            _ => self
                .nominal_application(target)
                .map_or_else(Vec::new, |application| application.arguments),
        };
        let target_bindings = original
            .owner_parameters
            .iter()
            .chain(&original.callable_parameters)
            .map(|parameter| parameter.id)
            .zip(target_arguments)
            .collect::<Vec<_>>();
        let mut signature = original.clone();
        for parameter in &mut signature.value_parameters {
            parameter.ty = self.instantiate_method_ty(parameter.ty, &target_bindings);
            if let ValueParameterCalling::Vararg {
                element_type,
                array_type,
                ..
            } = &mut parameter.calling
            {
                *element_type = self.instantiate_method_ty(*element_type, &target_bindings);
                *array_type = self.instantiate_method_ty(*array_type, &target_bindings);
            }
        }
        signature.owner_parameters.clear();
        signature.callable_parameters = parameters;
        signature.return_type = target;
        Some(AliasConstructorSignature {
            signature,
            explicit: arguments,
            target_bindings,
        })
    }
}
