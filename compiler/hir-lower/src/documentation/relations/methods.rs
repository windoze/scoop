use super::*;

impl Lowerer {
    pub(super) fn documentation_inherited_method(
        &mut self,
        candidate: crate::CallableCandidate,
    ) -> Relation {
        let function = candidate.function;
        let original = &self.signatures[&function];
        let mut own = original.type_params[original.owner_type_param_count..].to_vec();
        let arguments = self.callable_candidate_owner_arguments(&candidate);
        let mut bindings = original.type_params[..original.owner_type_param_count]
            .iter()
            .zip(&arguments)
            .map(|(parameter, ty)| (parameter.id, *ty))
            .collect::<Vec<_>>();
        bindings.extend(own.iter().map(|parameter| {
            (
                parameter.id,
                self.intern_type(hir::Type::Param(parameter.id)),
            )
        }));
        for parameter in &mut own {
            self.documentation_substitute_parameter(parameter, &bindings);
        }
        let signature = self.instantiated_signature(function, &arguments, &own);
        let mut parameters = self.type_params_in_scope.clone();
        parameters.extend(own.iter().cloned());
        let generics =
            crate::call_resolution::diagnostics::render_type_parameters(self, &own, &parameters);
        let values = signature
            .params
            .iter()
            .map(|parameter| {
                let (prefix, ty, suffix) = match &parameter.calling {
                    crate::FnParamCalling::Required => ("", parameter.ty, ""),
                    crate::FnParamCalling::Default { .. } => ("", parameter.ty, " = …"),
                    crate::FnParamCalling::Vararg {
                        element_ty,
                        omission,
                    } => (
                        "vararg ",
                        *element_ty,
                        if matches!(omission, crate::FnVarargOmission::Default { .. }) {
                            " = …"
                        } else {
                            ""
                        },
                    ),
                };
                format!(
                    "{prefix}{}: {}{suffix}",
                    parameter.name.text,
                    self.type_name_with_params(ty, &parameters)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let context = signature
            .context_parameters
            .iter()
            .map(|parameter| self.type_name_with_params(parameter.ty, &parameters))
            .collect::<Vec<_>>();
        let context = if context.is_empty() {
            String::new()
        } else {
            format!("context({}) ", context.join(", "))
        };
        let name = self.source_function_name(function);
        let effects = format!(
            "{}{}{}{}",
            if signature.attributes.safety == hir::Safety::Unsafe {
                "@Unsafe "
            } else {
                ""
            },
            if signature.attributes.gc_effect == hir::GcEffect::NoGc {
                "@NoGC "
            } else {
                ""
            },
            if signature.modifiers.operator.is_some()
                || signature.modifiers.property_delegate_operator.is_some()
            {
                "operator "
            } else {
                ""
            },
            if signature.modifiers.is_infix {
                "infix "
            } else {
                ""
            }
        );
        let origin = match candidate.owner {
            crate::CallableCandidateOwner::Method(owner) => {
                self.type_name(self.method_owner_type(owner))
            }
            crate::CallableCandidateOwner::TypeOwned(owner) => self.type_name(owner),
            crate::CallableCandidateOwner::Function { .. } => {
                unreachable!("a nominal member has a declaring type")
            }
        };
        Relation {
            kind: RelationKind::Inherited,
            target: Symbol::Current(self.documentation_function_target(function)),
            signature: format!(
                "{context}{effects}{}fun {name}{generics}({values}): {}{}",
                if signature.is_suspend { "suspend " } else { "" },
                self.type_name_with_params(signature.return_ty, &parameters),
                self.documentation_constraints(&own, &parameters)
            ),
            origin,
            visibility: self.functions[function].access.declared,
            hidden: false,
        }
    }
}
