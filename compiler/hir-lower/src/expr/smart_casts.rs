use super::*;
use std::collections::BTreeMap;

mod properties;
mod reads;

pub(crate) type SmartCastFacts = BTreeMap<hir::BindingId, Vec<TypeId>>;

impl Lowerer {
    /// Conjunctive paths retain every constraint; alternative paths retain
    /// constraints implied by both. Binding identities survive capture.
    pub(crate) fn resolve_smart_casts(
        &mut self,
        cond: &ast::Expr,
        outcome: bool,
    ) -> SmartCastFacts {
        match cond {
            ast::Expr::Is {
                operand,
                ty,
                negated,
                ..
            } if outcome != *negated => {
                let ast::Expr::Var(name) = operand.as_ref() else {
                    return BTreeMap::new();
                };
                let Some((binding, declared)) = self.stable_smart_cast_binding(&name.text) else {
                    return BTreeMap::new();
                };
                let Some(narrowed) = self.resolve_type_ref(ty) else {
                    return BTreeMap::new();
                };
                if self.is_subtype(declared, narrowed) {
                    return BTreeMap::new();
                }
                BTreeMap::from([(binding, vec![narrowed])])
            }
            ast::Expr::Unary {
                op: ast::UnOp::Not,
                operand,
                ..
            } => self.resolve_smart_casts(operand, !outcome),
            ast::Expr::Binary {
                op: ast::BinOp::And | ast::BinOp::Or,
                lhs,
                rhs,
                ..
            } => {
                let ast::Expr::Binary { op, .. } = cond else {
                    unreachable!()
                };
                let enter_rhs = *op == ast::BinOp::And;
                let left = self.resolve_smart_casts(lhs, enter_rhs);
                let right = self
                    .with_smart_casts(left.clone(), |this| this.resolve_smart_casts(rhs, outcome));
                let both = self.join_smart_casts(left, right);
                if outcome == enter_rhs {
                    both
                } else {
                    let short = self.resolve_smart_casts(lhs, outcome);
                    self.common_smart_casts(short, both)
                }
            }
            _ => BTreeMap::new(),
        }
    }

    pub(crate) fn join_smart_casts(
        &mut self,
        mut lhs: SmartCastFacts,
        rhs: SmartCastFacts,
    ) -> SmartCastFacts {
        for (binding, types) in rhs {
            let known = lhs.entry(binding).or_default();
            for ty in types {
                self.add_smart_cast_type(known, ty);
            }
        }
        lhs
    }

    pub(crate) fn common_smart_casts(
        &mut self,
        lhs: SmartCastFacts,
        rhs: SmartCastFacts,
    ) -> SmartCastFacts {
        let mut result = BTreeMap::new();
        for (binding, left) in lhs {
            let Some(right) = rhs.get(&binding) else {
                continue;
            };
            let mut common = Vec::new();
            for left in left {
                for right in right {
                    let ty = self.least_upper_bound(&[left, *right]);
                    if !self.types_equal(ty, self.any) {
                        self.add_smart_cast_type(&mut common, ty);
                    }
                }
            }
            if !common.is_empty() {
                result.insert(binding, common);
            }
        }
        result
    }

    fn add_smart_cast_type(&mut self, known: &mut Vec<TypeId>, ty: TypeId) {
        if known.iter().any(|&before| self.is_subtype(before, ty)) {
            return;
        }
        known.retain(|&before| !self.is_subtype(ty, before));
        known.push(ty);
    }

    pub(crate) fn with_smart_casts<T>(
        &mut self,
        narrowings: SmartCastFacts,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        if narrowings.is_empty() {
            return f(self);
        }
        let saved = self.smart_casts.clone();
        self.smart_casts = self.join_smart_casts(saved.clone(), narrowings);
        let result = f(self);
        self.smart_casts = saved;
        result
    }
}
