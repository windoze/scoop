use super::*;

impl Lowerer {
    pub(super) fn documentation_substitute_parameter(
        &mut self,
        parameter: &mut hir::TypeParamDecl,
        bindings: &[(hir::TypeParamId, hir::TypeId)],
    ) {
        let mut bounds = |bounds: &mut hir::TypeParamBounds| {
            if let hir::TypeParamBounds::Nominal(bounds) = bounds {
                if let Some(class) = &mut bounds.class {
                    class.ty = self.instantiate_method_ty(class.ty, bindings);
                }
                for interface in &mut bounds.interfaces {
                    interface.ty = self.instantiate_method_ty(interface.ty, bindings);
                }
            }
        };
        bounds(&mut parameter.bounds);
        if let Some(companion) = &mut parameter.companion {
            bounds(&mut companion.bounds);
        }
        for interface in &mut parameter.excluded_interfaces {
            interface.ty = self.instantiate_method_ty(interface.ty, bindings);
        }
        if let Some(companion) = &mut parameter.companion {
            for interface in &mut companion.excluded_interfaces {
                interface.ty = self.instantiate_method_ty(interface.ty, bindings);
            }
        }
        for requirement in &mut parameter.requirements {
            requirement
                .contract
                .map_types(|ty| self.instantiate_method_ty(ty, bindings));
        }
    }

    pub(super) fn documentation_constraints(
        &self,
        parameters: &[hir::TypeParamDecl],
        scope: &[hir::TypeParamDecl],
    ) -> String {
        let mut clauses = Vec::new();
        for parameter in parameters {
            if let Some(companion) = &parameter.companion {
                let mut display = parameter.clone();
                display.name = format!("{}.Companion", parameter.name);
                display.bounds = companion.bounds.clone();
                display.excluded_interfaces = companion.excluded_interfaces.clone();
                let clause = crate::call_resolution::diagnostics::render_type_parameters(
                    self,
                    &[display],
                    scope,
                );
                clauses.push(clause[1..clause.len() - 1].to_owned());
            }
            for requirement in &parameter.requirements {
                clauses.push(self.documentation_requirement(&requirement.contract, scope));
            }
        }
        if clauses.is_empty() {
            String::new()
        } else {
            format!(" where {}", clauses.join(", "))
        }
    }

    fn documentation_requirement(
        &self,
        contract: &hir::MemberRequirementContract,
        scope: &[hir::TypeParamDecl],
    ) -> String {
        let ty = |ty| self.type_name_with_params(ty, scope);
        let arguments = contract
            .arguments
            .iter()
            .enumerate()
            .map(|(index, argument)| {
                format!(
                    "{}{}a{index}: {}",
                    argument
                        .name
                        .as_ref()
                        .map(|name| format!("{name} = "))
                        .unwrap_or_default(),
                    if argument.spread { "*" } else { "" },
                    ty(argument.ty)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let effects = format!(
            "{}{}{}",
            if contract.effects.safety == hir::Safety::Unsafe {
                "@Unsafe "
            } else {
                ""
            },
            if contract.effects.gc_effect == hir::GcEffect::NoGc {
                "@NoGC "
            } else {
                ""
            },
            if contract.effects.is_suspend {
                "suspend "
            } else {
                ""
            }
        );
        match &contract.call {
            hir::RequiredCallKind::Constructor => format!(
                "requires {{ {effects}{}({arguments}) }}",
                ty(contract.subject)
            ),
            hir::RequiredCallKind::Method {
                name,
                type_arguments,
            } => {
                let arguments_of_type = type_arguments
                    .iter()
                    .map(|argument| match argument {
                        hir::RequiredTypeArgument::Explicit(value) => ty(*value),
                        hir::RequiredTypeArgument::Infer => "_".into(),
                    })
                    .collect::<Vec<_>>();
                let generics = if arguments_of_type.is_empty() {
                    String::new()
                } else {
                    format!("<{}>", arguments_of_type.join(", "))
                };
                format!(
                    "requires(x: {}) {{ {effects}x.{name}{generics}({arguments}): {} }}",
                    ty(contract.subject),
                    ty(contract.result)
                )
            }
        }
    }
}
