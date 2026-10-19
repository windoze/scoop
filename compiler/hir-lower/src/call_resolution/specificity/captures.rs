use super::*;
use crate::Type;
use std::collections::HashSet;

impl Lowerer {
    /// Alias expansion can leave rigid lexical parameters inside a declaration's
    /// input types and bounds. Give those existing identities fixed equations,
    /// while the declaration's own binders remain ordinary inference variables.
    pub(super) fn bind_forwarding_captures(
        &mut self,
        session: &mut InferenceSession,
        declaration: DeclarationForwardingView<'_>,
    ) {
        let mut pending = declaration.parameter_types.to_vec();
        for parameter in declaration
            .owner_parameters
            .iter()
            .chain(declaration.callable_parameters)
        {
            pending.extend(
                parameter
                    .requirements
                    .iter()
                    .flat_map(|requirement| requirement.contract.types()),
            );
            if let Some(companion) = &parameter.companion {
                pending.extend(companion.excluded_interfaces.iter().map(|bound| bound.ty));
                if let hir::TypeParamBounds::Nominal(bounds) = &companion.bounds {
                    pending.extend(bounds.in_source_order().into_iter().map(|bound| bound.ty()));
                }
            }
            pending.extend(parameter.excluded_interfaces.iter().map(|bound| bound.ty));
            pending.extend(
                parameter
                    .nominal_bounds_in_source_order()
                    .into_iter()
                    .map(|bound| match bound {
                        hir::NominalBoundRef::Class(bound) => bound.ty,
                        hir::NominalBoundRef::Interface(bound) => bound.ty,
                    }),
            );
        }
        let mut visited = HashSet::new();
        let mut used = HashSet::new();
        while let Some(ty) = pending.pop() {
            if !visited.insert(ty) {
                continue;
            }
            if let Some(application) = self.nominal_application(ty) {
                pending.extend(application.arguments);
                continue;
            }
            match &self.types[ty] {
                Type::Param(id) | Type::Companion(id) => {
                    used.insert(*id);
                }
                Type::Tuple(elements) => pending.extend(elements),
                Type::Ptr(pointee) => pending.push(*pointee),
                Type::Function(id) | Type::FunPtr(id) => {
                    let signature = &self.function_types[*id];
                    pending.extend(signature.parameters());
                    pending.push(signature.return_type);
                }
                _ => {}
            }
        }
        let captures = self
            .type_params_in_scope
            .iter()
            .filter(|p| used.contains(&p.id) && session.variable_for(p.id).is_none())
            .cloned()
            .collect::<Vec<_>>();
        if captures.is_empty() {
            return;
        }
        session.add_environment(&captures, &[]);
        for parameter in captures {
            let variable = session
                .variable_for(parameter.id)
                .expect("the captured parameter was installed");
            let ty = self.intern_type(Type::Param(parameter.id));
            session.push(
                Constraint::Equal(variable.into(), TypeTerm::Rigid(ty)),
                ConstraintOrigin::Specificity,
            );
        }
    }
}
