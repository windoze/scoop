//! Declaration-site facts for an inherited implementation's substituted domain.

use super::*;

#[derive(Clone, Default)]
pub(super) struct ConditionalAssumptions {
    parents: Vec<(TypeId, TypeId)>,
    kinds: Vec<(TypeId, hir::TypeParamKind)>,
    companions: Vec<TypeId>,
}

impl Lowerer {
    pub(crate) fn with_conditional_interface_assumptions<R>(
        &mut self,
        condition: &hir::ConditionalInterfaceApplication,
        operation: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let declaration = self
            .declaration_conditional_interfaces(condition.owner)
            .iter()
            .find(|declaration| declaration.id() == condition.declaration)
            .expect("an inherited condition retains its original declaration")
            .clone();
        let previous = self.conditional_interfaces.assumptions.clone();
        // Register all projection existence facts before substituting bounds.
        for (parameter, &argument) in declaration.parameters.iter().zip(&condition.arguments) {
            if parameter.companion.is_some() {
                self.conditional_interfaces
                    .assumptions
                    .companions
                    .push(argument);
            }
        }
        for (parameter, &argument) in declaration.parameters.iter().zip(&condition.arguments) {
            self.assume_conditional_bounds(argument, &parameter.bounds, &condition.arguments);
            if let Some(companion) = &parameter.companion
                && let Ok(subject) = self.projected_companion_type(argument)
            {
                self.assume_conditional_bounds(subject, &companion.bounds, &condition.arguments);
            }
        }
        let result = operation(self);
        self.conditional_interfaces.assumptions = previous;
        result
    }

    fn assume_conditional_bounds(
        &mut self,
        subject: TypeId,
        bounds: &hir::TypeParamBounds,
        arguments: &[TypeId],
    ) {
        let kind = match bounds {
            hir::TypeParamBounds::Unconstrained => hir::TypeParamKind::Any,
            hir::TypeParamBounds::Value { .. } => hir::TypeParamKind::Value,
            hir::TypeParamBounds::Ref { .. } => hir::TypeParamKind::Ref,
            hir::TypeParamBounds::Nominal(bounds) => {
                for bound in bounds.in_source_order() {
                    let bound = self.instantiate_ty(bound.ty(), arguments);
                    self.conditional_interfaces
                        .assumptions
                        .parents
                        .push((subject, bound));
                }
                if bounds.class.is_some() {
                    hir::TypeParamKind::Ref
                } else {
                    hir::TypeParamKind::Any
                }
            }
        };
        if kind != hir::TypeParamKind::Any {
            self.conditional_interfaces
                .assumptions
                .kinds
                .push((subject, kind));
        }
    }

    pub(crate) fn assumed_conditional_parents(&self, subject: TypeId) -> Vec<TypeId> {
        self.conditional_interfaces
            .assumptions
            .parents
            .iter()
            .filter_map(|&(candidate, bound)| self.types_equal(subject, candidate).then_some(bound))
            .collect()
    }

    pub(crate) fn assumed_conditional_kind(
        &self,
        subject: TypeId,
        kind: hir::TypeParamKind,
    ) -> bool {
        self.conditional_interfaces
            .assumptions
            .kinds
            .iter()
            .any(|&(candidate, assumed)| kind == assumed && self.types_equal(subject, candidate))
    }

    pub(crate) fn assumed_conditional_companion(&self, subject: TypeId) -> bool {
        self.conditional_interfaces
            .assumptions
            .companions
            .iter()
            .any(|&candidate| self.types_equal(subject, candidate))
    }
}
