use super::*;

impl Lowerer {
    pub(super) fn declare_companion_conditions(
        &mut self,
        parameters: &mut [hir::TypeParamDecl],
        own: &std::collections::HashMap<String, usize>,
        clause: Option<&ast::WhereClause>,
        allowed: bool,
    ) {
        if !allowed {
            return;
        }
        for constraint in clause.into_iter().flat_map(|clause| &clause.constraints) {
            let ast::TypeConstraintSubject::Companion(parameter) = &constraint.subject else {
                continue;
            };
            if let Some(&index) = own.get(&parameter.text) {
                parameters[index]
                    .companion
                    .get_or_insert(hir::CompanionBounds {
                        bounds: hir::TypeParamBounds::Unconstrained,
                        excluded_interfaces: Vec::new(),
                        span: constraint.span,
                    });
            }
        }
        self.type_params_in_scope = parameters.to_vec();
    }
}

pub(super) fn subject_name(parameter: &hir::TypeParamDecl, companion: bool) -> String {
    if companion {
        format!("{}.Companion", parameter.name)
    } else {
        parameter.name.clone()
    }
}

pub(super) fn subject_bounds_mut(
    parameter: &mut hir::TypeParamDecl,
    companion: bool,
) -> &mut hir::TypeParamBounds {
    if companion {
        &mut parameter
            .companion
            .as_mut()
            .expect("projection conditions are declared before resolving their bounds")
            .bounds
    } else {
        &mut parameter.bounds
    }
}

pub(super) fn subject_exclusions_mut(
    parameter: &mut hir::TypeParamDecl,
    companion: bool,
) -> &mut Vec<hir::InterfaceUpperBound> {
    if companion {
        &mut parameter
            .companion
            .as_mut()
            .expect("projection conditions are declared before resolving their bounds")
            .excluded_interfaces
    } else {
        &mut parameter.excluded_interfaces
    }
}
