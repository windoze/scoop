use super::*;

impl SmartFlow<'_> {
    pub(super) fn expression(
        &mut self,
        facts: SmartCastFacts,
        value: &hir::Expr,
    ) -> SmartCastExits {
        if let hir::ExprKind::Binary {
            op: hir::BinOp::And | hir::BinOp::Or,
            lhs,
            rhs,
        } = &value.kind
        {
            let hir::ExprKind::Binary { op, .. } = &value.kind else {
                unreachable!()
            };
            return self.short_circuit(facts, *op == hir::BinOp::And, lhs, rhs);
        }
        let mut exits = Self::normal(facts);
        for operand in operands::operands(value) {
            if let Some(facts) = exits.remove(&Outcome::Fallthrough) {
                let next = self.expression(facts, operand);
                self.merge(&mut exits, next);
            }
        }
        let captures = match value.kind {
            hir::ExprKind::Lambda(id) => self.state.lambdas[id].captures.clone(),
            hir::ExprKind::AnonymousFunction(id) => {
                self.state.anonymous_functions[id].captures.clone()
            }
            hir::ExprKind::CallableReference(id) => {
                self.state.callable_references[id].captures.clone()
            }
            _ => Vec::new(),
        };
        for capture in captures {
            if let Some(facts) = exits.remove(&Outcome::Fallthrough) {
                let next = self.expression(facts, &capture.source);
                self.merge(&mut exits, next);
            }
        }
        if let Some(mut facts) = exits.remove(&Outcome::Fallthrough) {
            self.check_read(&facts, value);
            if let hir::ExprKind::AddressOf(hir::Place::Local(local)) = value.kind {
                facts.expose_address(self.state.locals[local].binding);
            }
            if self.state.is_nothing_ty(value.ty) {
                self.insert(&mut exits, Outcome::Throw, facts);
            } else {
                if operands::may_throw(value) {
                    self.insert(&mut exits, Outcome::Throw, facts.clone());
                }
                exits.insert(Outcome::Fallthrough, facts);
            }
        }
        exits
    }

    fn short_circuit(
        &mut self,
        facts: SmartCastFacts,
        enter: bool,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> SmartCastExits {
        let mut exits = self.expression(facts, lhs);
        if let Some(facts) = exits.remove(&Outcome::Fallthrough) {
            if let Some(short) = self.state.refine_flow_condition(facts.clone(), lhs, !enter) {
                self.insert(&mut exits, Outcome::Fallthrough, short);
            }
            if let Some(right) = self.state.refine_flow_condition(facts, lhs, enter) {
                let next = self.expression(right, rhs);
                self.merge(&mut exits, next);
            }
        }
        exits
    }

    fn check_read(&mut self, facts: &SmartCastFacts, value: &hir::Expr) {
        if !self.check_reads
            || !matches!(
                value.kind,
                hir::ExprKind::Local(_) | hir::ExprKind::Unbox(_)
            )
        {
            return;
        }
        let Some(local) = source_local(value) else {
            return;
        };
        let local = &self.state.locals[local];
        if !local.mutable {
            return;
        }
        let (binding, declared) = (local.binding, local.ty);
        let name = local.name.clone();
        if self.state.is_subtype(declared, value.ty)
            || facts
                .get(&binding)
                .is_some_and(|types| types.iter().any(|&ty| self.state.is_subtype(ty, value.ty)))
        {
            return;
        }
        let message = format!(
            "smart cast of mutable local `{name}` to {} is not valid on every incoming path",
            self.state.type_name(value.ty)
        );
        if !self
            .state
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.span == Some(value.span) && diagnostic.message == message)
        {
            self.state.error(value.span, message);
        }
    }
}

fn source_local(value: &hir::Expr) -> Option<hir::LocalId> {
    match &value.kind {
        hir::ExprKind::Local(local) => Some(*local),
        hir::ExprKind::Unbox(source)
        | hir::ExprKind::Upcast(source)
        | hir::ExprKind::Box(source) => source_local(source),
        _ => None,
    }
}
