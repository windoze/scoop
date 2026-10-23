use super::*;
use crate::call_resolution::constraints::{
    Constraint, ConstraintFailureKind, ConstraintOrigin, InferenceSession, TypeTerm,
};

impl Lowerer {
    pub(super) fn sealed_children(&mut self, parent: TypeId) -> Option<Vec<Family>> {
        let subtypes = self.sealed_subtypes(parent)?.to_vec();
        let parent_template = self.nominal_application(parent)?.template;
        let mut families = Vec::new();
        for subtype in subtypes {
            let parameters = match self.nominal_declaration_parameters(subtype) {
                Ok(parameters) => parameters,
                Err(error) => {
                    self.diagnostics.push(ast::Diagnostic::without_span(
                        ast::DiagnosticSeverity::Error,
                        self.current_file,
                        format!("cannot load sealed subtype: {error:?}"),
                    ));
                    return None;
                }
            };
            let arguments = parameters
                .iter()
                .map(|parameter| self.intern_type(Type::Param(parameter.id)))
                .collect();
            self.apply_nominal_type(subtype, arguments)
                .expect("a loaded nominal accepts its original binders");
            let mut domains = vec![(None, parameters)];
            for declaration in self.declaration_conditional_interfaces(subtype) {
                if self
                    .nominal_application(declaration.interface)
                    .is_some_and(|application| application.template == parent_template)
                {
                    domains.push((
                        Some((declaration.id(), declaration.interface)),
                        declaration.parameters.clone(),
                    ));
                }
            }
            for (implementation, original_parameters) in domains {
                let mut parameters = original_parameters.clone();
                let fresh = parameters
                    .iter_mut()
                    .enumerate()
                    .map(|(index, parameter)| {
                        let original = parameter.id;
                        parameter.id = self.fresh_type_param(index);
                        (original, self.intern_type(Type::Param(parameter.id)))
                    })
                    .collect::<Vec<_>>();
                self.substitute_family_bounds(&mut parameters, &fresh);
                let arguments = fresh.iter().map(|(_, ty)| *ty).collect::<Vec<_>>();
                let ty = self
                    .apply_nominal_type(subtype, fresh.iter().map(|(_, ty)| *ty).collect())
                    .expect("a loaded nominal accepts its original binders");
                let condition =
                    implementation.map(|(declaration, _)| hir::ConditionalInterfaceApplication {
                        declaration,
                        owner: subtype,
                        arguments: arguments.clone(),
                    });
                let scope_len = self.type_params_in_scope.len();
                self.type_params_in_scope.extend(parameters.iter().cloned());
                let bases = match implementation {
                    Some((_, interface)) => vec![self.instantiate_ty(interface, &arguments)],
                    None => self.direct_nominal_supertypes(ty),
                };
                for base in bases {
                    if self
                        .nominal_application(base)
                        .is_none_or(|base| base.template != parent_template)
                    {
                        continue;
                    }
                    if let Some(family) = self.sealed_child_application(
                        parent,
                        base,
                        &original_parameters,
                        Family {
                            ty,
                            parameters: parameters.clone(),
                            condition: condition.clone(),
                        },
                    ) {
                        families.push(family);
                    }
                }
                self.type_params_in_scope.truncate(scope_len);
            }
        }
        Some(families)
    }

    fn sealed_child_application(
        &mut self,
        parent: TypeId,
        base: TypeId,
        original_parameters: &[hir::TypeParamDecl],
        family: Family,
    ) -> Option<Family> {
        let Family {
            ty,
            parameters,
            condition,
        } = family;
        let mut session = InferenceSession::new();
        let environment = session.add_environment(&parameters, &[]);
        session.push(
            Constraint::Equal(TypeTerm::Type(base), TypeTerm::Rigid(parent)),
            ConstraintOrigin::ExpectedResult,
        );
        let Ok(solution) = self.solve_constraints_partially(&session, environment) else {
            if self.sealed_applications_may_equal(base, parent, &parameters) {
                return Some(Family {
                    ty,
                    parameters: parameters.to_vec(),
                    condition,
                });
            }
            return None;
        };
        let bindings = parameters
            .iter()
            .zip(&solution.owner)
            .map(|(parameter, value)| {
                (
                    parameter.id,
                    value.unwrap_or_else(|| self.intern_type(Type::Param(parameter.id))),
                )
            })
            .collect::<Vec<_>>();
        let arguments = bindings.iter().map(|(_, ty)| *ty).collect::<Vec<_>>();
        if self.conditional_parameters_are_impossible(original_parameters, &arguments) {
            return None;
        }
        let instantiated_base = self.instantiate_method_ty(base, &bindings);
        if !self.types_equal(instantiated_base, parent) {
            if self.sealed_applications_may_equal(base, parent, &parameters) {
                return Some(Family {
                    ty,
                    parameters: parameters.to_vec(),
                    condition,
                });
            }
            return None;
        }
        let mut instantiated = parameters.to_vec();
        self.substitute_family_bounds(&mut instantiated, &bindings);
        let unresolved = instantiated
            .iter()
            .zip(&solution.owner)
            .filter_map(|(parameter, value)| value.is_none().then_some(parameter.clone()))
            .collect::<Vec<_>>();
        Some(Family {
            ty: self.instantiate_method_ty(ty, &bindings),
            parameters: unresolved,
            condition: condition.map(|mut condition| {
                condition.arguments = arguments;
                condition
            }),
        })
    }

    fn sealed_applications_may_equal(
        &mut self,
        base: TypeId,
        parent: TypeId,
        parameters: &[hir::TypeParamDecl],
    ) -> bool {
        let mut identities = parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect::<std::collections::HashSet<_>>();
        let outer = self
            .type_params_in_scope
            .iter()
            .filter(|parameter| identities.insert(parameter.id))
            .cloned()
            .collect::<Vec<_>>();
        let mut session = InferenceSession::new();
        session.add_environment(parameters, &outer);
        self.add_declaration_bounds(&mut session, parameters.iter().chain(&outer));
        session.push(
            Constraint::Equal(TypeTerm::Type(base), TypeTerm::Type(parent)),
            ConstraintOrigin::ExpectedResult,
        );
        match self.solve_constraints(&session) {
            Ok(_) => true,
            Err(failure) => matches!(
                failure.kind,
                ConstraintFailureKind::NoUniqueSolution { .. }
                    | ConstraintFailureKind::UnresolvedTerm(_)
                    | ConstraintFailureKind::NonConcreteApplication(_)
            ),
        }
    }

    fn substitute_family_bounds(
        &mut self,
        parameters: &mut [hir::TypeParamDecl],
        bindings: &[(hir::TypeParamId, TypeId)],
    ) {
        for parameter in parameters {
            for bounds in std::iter::once(&mut parameter.bounds).chain(
                parameter
                    .companion
                    .iter_mut()
                    .map(|companion| &mut companion.bounds),
            ) {
                let hir::TypeParamBounds::Nominal(bounds) = bounds else {
                    continue;
                };
                if let Some(bound) = &mut bounds.class {
                    bound.ty = self.instantiate_method_ty(bound.ty, bindings);
                }
                for bound in &mut bounds.interfaces {
                    bound.ty = self.instantiate_method_ty(bound.ty, bindings);
                }
            }
        }
    }

    pub(super) fn family_may_match(&mut self, family: TypeId, target: TypeId) -> bool {
        if self.could_hold(family, target) {
            return true;
        }
        if !self.type_contains_param(family) && !self.type_contains_param(target) {
            return false;
        }
        // Different rigid parameters need not differ after monomorphization.
        // Their nominal ancestors still determine whether overlap is possible.
        self.has_family_ancestor(family, target) || self.has_family_ancestor(target, family)
    }

    fn has_family_ancestor(&mut self, ty: TypeId, target: TypeId) -> bool {
        let Some(target) = self.nominal_application(target) else {
            return false;
        };
        let mut pending = vec![ty];
        let mut seen = std::collections::HashSet::new();
        while let Some(ty) = pending.pop() {
            if !seen.insert(ty) {
                continue;
            }
            if self
                .nominal_application(ty)
                .is_some_and(|application| application.template == target.template)
            {
                return true;
            }
            pending.extend(self.direct_nominal_supertypes(ty));
        }
        false
    }
}
