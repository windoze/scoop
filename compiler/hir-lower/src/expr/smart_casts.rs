use super::*;

type Facts = Vec<(hir::LocalId, TypeId)>;

impl Lowerer {
    /// Facts describe execution paths, including the two ways a short-circuit
    /// expression can produce its result. Alternative paths retain only their
    /// common constraints; conjunctive paths keep the most precise view.
    pub(crate) fn resolve_smart_casts(&mut self, cond: &ast::Expr, outcome: bool) -> Facts {
        match cond {
            ast::Expr::Is {
                operand,
                ty,
                negated,
                ..
            } if outcome != *negated => {
                let ast::Expr::Var(name) = operand.as_ref() else {
                    return Vec::new();
                };
                let Some(local) = self.scopes.lookup(&name.text) else {
                    return Vec::new();
                };
                if self.locals[local].mutable
                    || self
                        .local_delegate_plans
                        .contains_key(&self.locals[local].binding)
                {
                    return Vec::new();
                }
                let Some(narrowed) = self.resolve_type_ref(ty) else {
                    return Vec::new();
                };
                let declared = self.locals[local].ty;
                if self.types_equal(narrowed, declared) || !self.is_subtype(narrowed, declared) {
                    return Vec::new();
                }
                vec![(local, narrowed)]
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
            _ => Vec::new(),
        }
    }

    fn join_smart_casts(&mut self, mut lhs: Facts, rhs: Facts) -> Facts {
        for (local, ty) in rhs {
            match lhs.iter_mut().find(|(known, _)| *known == local) {
                Some((_, known)) if self.is_subtype(ty, *known) => *known = ty,
                Some(_) => {}
                None => lhs.push((local, ty)),
            }
        }
        lhs
    }

    fn common_smart_casts(&mut self, lhs: Facts, rhs: Facts) -> Facts {
        lhs.into_iter()
            .filter_map(|(local, left)| {
                let (_, right) = rhs.iter().find(|(known, _)| *known == local)?;
                let ty = self.least_upper_bound(&[left, *right]);
                (!self.types_equal(ty, self.locals[local].ty)).then_some((local, ty))
            })
            .collect()
    }

    pub(crate) fn with_smart_casts<T>(
        &mut self,
        narrowings: Facts,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        if narrowings.is_empty() {
            return f(self);
        }
        let saved = self.smart_casts.clone();
        for (local, ty) in narrowings {
            if self
                .smart_casts
                .get(&local)
                .copied()
                .is_none_or(|known| self.is_subtype(ty, known))
            {
                self.smart_casts.insert(local, ty);
            }
        }
        let result = f(self);
        self.smart_casts = saved;
        result
    }
}
