use super::*;

impl Lowerer {
    pub(super) fn editing_parameter_label(&self, id: hir::TypeParamId) -> Option<String> {
        let groups = self
            .signatures
            .values()
            .map(|signature| signature.type_params.as_slice())
            .chain(
                self.type_aliases
                    .iter()
                    .map(|(_, alias)| alias.type_params.as_slice()),
            )
            .chain(
                self.classes
                    .iter()
                    .map(|(_, owner)| owner.type_params.as_slice()),
            )
            .chain(
                self.structs
                    .iter()
                    .map(|(_, owner)| owner.type_params.as_slice()),
            )
            .chain(
                self.enums
                    .iter()
                    .map(|(_, owner)| owner.type_params.as_slice()),
            )
            .chain(
                self.interfaces
                    .iter()
                    .map(|(_, owner)| owner.type_params.as_slice()),
            );
        let (parameters, parameter) = groups
            .filter_map(|parameters| {
                parameters
                    .iter()
                    .find(|parameter| parameter.id == id)
                    .map(|parameter| (parameters, parameter))
            })
            .next()?;
        let mut label = format!("type parameter {}", parameter.name);
        let bounds = self.editing_parameter_bounds(&parameter.bounds, parameters);
        if !bounds.is_empty() {
            label.push_str(&format!(": {}", bounds.join(" & ")));
        }
        let mut constraints = Vec::new();
        for excluded in &parameter.excluded_interfaces {
            constraints.push(format!(
                "{}: !{}",
                parameter.name,
                self.type_name_with_params(excluded.ty, parameters)
            ));
        }
        if let Some(companion) = &parameter.companion {
            let bounds = self.editing_parameter_bounds(&companion.bounds, parameters);
            constraints.push(if bounds.is_empty() {
                format!("{}.Companion", parameter.name)
            } else {
                format!("{}.Companion: {}", parameter.name, bounds.join(" & "))
            });
            for excluded in &companion.excluded_interfaces {
                constraints.push(format!(
                    "{}.Companion: !{}",
                    parameter.name,
                    self.type_name_with_params(excluded.ty, parameters)
                ));
            }
        }
        for requirement in &parameter.requirements {
            constraints.push(self.editing_requirement_label(&requirement.contract, parameters));
        }
        if !constraints.is_empty() {
            label.push_str(&format!("\nwhere {}", constraints.join(", ")));
        }
        Some(label)
    }

    fn editing_parameter_bounds(
        &self,
        bounds: &hir::TypeParamBounds,
        parameters: &[hir::TypeParamDecl],
    ) -> Vec<String> {
        match bounds {
            hir::TypeParamBounds::Unconstrained => Vec::new(),
            hir::TypeParamBounds::Value { .. } => vec!["value".into()],
            hir::TypeParamBounds::Ref { .. } => vec!["ref".into()],
            hir::TypeParamBounds::Nominal(bounds) => bounds
                .in_source_order()
                .iter()
                .map(|bound| self.type_name_with_params(bound.ty(), parameters))
                .collect(),
        }
    }

    fn editing_requirement_label(
        &self,
        contract: &hir::MemberRequirementContract,
        parameters: &[hir::TypeParamDecl],
    ) -> String {
        let ty = |ty| self.type_name_with_params(ty, parameters);
        let mut subject = ty(contract.subject);
        if let hir::RequiredCallKind::Method {
            name,
            type_arguments,
        } = &contract.call
        {
            subject.push_str(&format!(".{name}"));
            if !type_arguments.is_empty() {
                let arguments = type_arguments
                    .iter()
                    .map(|argument| match argument {
                        hir::RequiredTypeArgument::Explicit(argument) => ty(*argument),
                        hir::RequiredTypeArgument::Infer => "_".into(),
                    })
                    .collect::<Vec<_>>();
                subject.push_str(&format!("<{}>", arguments.join(", ")));
            }
        }
        let arguments = contract
            .arguments
            .iter()
            .map(|argument| {
                let name = argument
                    .name
                    .as_ref()
                    .map_or_else(String::new, |name| format!("{name} = "));
                let spread = if argument.spread { "*" } else { "" };
                format!("{name}{spread}{}", ty(argument.ty))
            })
            .collect::<Vec<_>>()
            .join(", ");
        let no_gc = if contract.effects.gc_effect == hir::GcEffect::NoGc {
            "@NoGC "
        } else {
            ""
        };
        let unsafe_ = if contract.effects.safety == hir::Safety::Unsafe {
            "unsafe "
        } else {
            ""
        };
        let suspend = if contract.effects.is_suspend {
            "suspend "
        } else {
            ""
        };
        format!(
            "{no_gc}{unsafe_}{suspend}{subject}({arguments}): {}",
            ty(contract.result)
        )
    }
}
