//! Exhaustiveness of declaration-closed nominal families.

use super::*;

mod applications;

struct Family {
    ty: TypeId,
    parameters: Vec<hir::TypeParamDecl>,
    condition: Option<hir::ConditionalInterfaceApplication>,
}

impl Lowerer {
    pub(super) fn prove_sealed_when(
        &mut self,
        when: &ast::When,
        subject: TypeId,
    ) -> Option<hir::ExhaustivenessProof> {
        let mut checks = Vec::new();
        for arm in &when.arms {
            if arm.guard.is_some() {
                continue;
            }
            match &arm.condition {
                ast::WhenArmCondition::Else => {
                    return Some(hir::ExhaustivenessProof::IrrefutableArm);
                }
                ast::WhenArmCondition::Conditions(conditions) => {
                    for condition in conditions.iter() {
                        if let ast::WhenCondition::Is { ty, negated, .. } = condition {
                            checks.push((self.resolve_type_ref(ty)?, *negated));
                        }
                    }
                }
                ast::WhenArmCondition::Case(_) => {}
            }
        }
        // Complements can cover a symbolic family without enumerating its
        // additional parameters. Guards were already excluded.
        if checks.iter().any(|&(negative, negated)| {
            negated
                && checks.iter().any(|&(other, negated)| {
                    if negated {
                        !self.family_may_match(negative, other)
                    } else {
                        self.is_subtype(negative, other)
                    }
                })
        }) {
            return Some(hir::ExhaustivenessProof::PatternMatrix);
        }
        let mut probe = self.clone();
        let family = Family {
            ty: subject,
            parameters: Vec::new(),
            condition: None,
        };
        if !probe.prove_sealed_family(family, &checks, when) {
            self.diagnostics.extend(
                probe
                    .diagnostics
                    .iter()
                    .skip(self.diagnostics.len())
                    .cloned(),
            );
            return None;
        }
        Some(hir::ExhaustivenessProof::PatternMatrix)
    }

    fn prove_sealed_family(
        &mut self,
        family: Family,
        checks: &[(TypeId, bool)],
        when: &ast::When,
    ) -> bool {
        let scope_len = self.type_params_in_scope.len();
        self.type_params_in_scope.extend(family.parameters);
        let result = match family.condition {
            Some(condition) if self.conditional_application_is_impossible(&condition) => true,
            Some(condition) => self.with_conditional_interface_assumptions(&condition, |this| {
                this.prove_sealed_family_domain(family.ty, checks, when)
            }),
            None => self.prove_sealed_family_domain(family.ty, checks, when),
        };
        self.type_params_in_scope.truncate(scope_len);
        result
    }

    fn prove_sealed_family_domain(
        &mut self,
        ty: TypeId,
        checks: &[(TypeId, bool)],
        when: &ast::When,
    ) -> bool {
        if checks.iter().any(|&(target, negated)| {
            if negated {
                !self.family_may_match(ty, target)
            } else {
                self.is_subtype(ty, target)
            }
        }) {
            return true;
        }
        if self.sealed_subtypes(ty).is_some() {
            // Declaration inheritance is acyclic. Keep each path's conditions
            // in scope: the same nominal reached under another domain is not
            // necessarily covered by the same arms.
            return self.sealed_children(ty).is_some_and(|children| {
                children
                    .into_iter()
                    .all(|child| self.prove_sealed_family(child, checks, when))
            });
        }
        let witness = self.type_name(ty);
        let guard = if when.arms.iter().any(|arm| arm.guard.is_some()) {
            "; guarded arms do not contribute to exhaustiveness"
        } else {
            ""
        };
        self.error(
            when.span,
            format!("non-exhaustive when: missing subtype family `{witness}`{guard}"),
        );
        false
    }
}
