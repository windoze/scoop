//! Exhaustiveness of declaration-closed nominal families.

use super::*;

mod applications;

struct Family {
    ty: TypeId,
    parameters: Vec<hir::TypeParamDecl>,
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
        let mut pending = vec![Family {
            ty: subject,
            parameters: Vec::new(),
        }];
        let mut seen = std::collections::HashSet::new();
        while let Some(family) = pending.pop() {
            if !seen.insert(family.ty) {
                continue;
            }
            let scope_len = probe.type_params_in_scope.len();
            probe
                .type_params_in_scope
                .extend(family.parameters.iter().cloned());
            let covered = checks.iter().any(|&(target, negated)| {
                if negated {
                    !probe.family_may_match(family.ty, target)
                } else {
                    probe.is_subtype(family.ty, target)
                }
            });
            if !covered {
                if probe.sealed_subtypes(family.ty).is_some() {
                    let Some(mut children) = probe.sealed_children(family.ty) else {
                        self.diagnostics.extend(
                            probe
                                .diagnostics
                                .iter()
                                .skip(self.diagnostics.len())
                                .cloned(),
                        );
                        return None;
                    };
                    for child in &mut children {
                        child.parameters.extend(family.parameters.iter().cloned());
                    }
                    pending.extend(children);
                } else {
                    let witness = probe.type_name(family.ty);
                    let guard = if when.arms.iter().any(|arm| arm.guard.is_some()) {
                        "; guarded arms do not contribute to exhaustiveness"
                    } else {
                        ""
                    };
                    self.error(
                        when.span,
                        format!("non-exhaustive when: missing subtype family `{witness}`{guard}"),
                    );
                    return None;
                }
            }
            probe.type_params_in_scope.truncate(scope_len);
        }
        Some(hir::ExhaustivenessProof::PatternMatrix)
    }
}
