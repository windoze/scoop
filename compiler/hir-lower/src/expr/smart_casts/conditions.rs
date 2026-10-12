use super::*;

impl Lowerer {
    pub(crate) fn refine_flow_condition(
        &mut self,
        mut facts: SmartCastFacts,
        condition: &hir::Expr,
        outcome: bool,
    ) -> Option<SmartCastFacts> {
        if self.is_nothing_ty(condition.ty) {
            return None;
        }
        match &condition.kind {
            hir::ExprKind::BoolLiteral(value) if *value != outcome => return None,
            hir::ExprKind::IsInstance { operand, check_ty } if outcome => {
                if let Some((binding, declared)) = self.flow_source_binding(operand)
                    && !self.is_subtype(declared, *check_ty)
                {
                    self.add_flow_type(&mut facts, binding, *check_ty);
                }
            }
            hir::ExprKind::Unary {
                op: hir::UnOp::Not,
                operand,
            }
            | hir::ExprKind::PrimitiveUnary {
                kind: hir::PrimitiveUnaryKind::BooleanNot,
                operand,
            } => {
                return self.refine_flow_condition(facts, operand, !outcome);
            }
            hir::ExprKind::Binary {
                op: op @ (hir::BinOp::And | hir::BinOp::Or),
                lhs,
                rhs,
            } => {
                let enter = *op == hir::BinOp::And;
                let both = self
                    .refine_flow_condition(facts.clone(), lhs, enter)
                    .and_then(|facts| self.refine_flow_condition(facts, rhs, outcome));
                if outcome == enter {
                    return both;
                }
                let short = self.refine_flow_condition(facts, lhs, outcome);
                return match (short, both) {
                    (Some(left), Some(right)) => Some(self.common_smart_casts(left, right)),
                    (left, right) => left.or(right),
                };
            }
            hir::ExprKind::Local(local) => {
                let binding = self.locals[*local].binding;
                if let Some(boolean) = facts.booleans.get(&binding) {
                    let added = if outcome { &boolean.yes } else { &boolean.no };
                    let added = added.clone()?;
                    self.join_flow_types(&mut facts, added);
                }
            }
            _ => return Some(facts),
        }
        Some(facts)
    }

    pub(crate) fn flow_source_binding(
        &self,
        value: &hir::Expr,
    ) -> Option<(hir::BindingId, TypeId)> {
        let source = match &value.kind {
            hir::ExprKind::Local(local) => {
                let local = &self.locals[*local];
                (!self.local_delegate_plans.contains_key(&local.binding))
                    .then_some((local.binding, local.ty))
            }
            hir::ExprKind::Capture(binding) => self
                .capture_contexts
                .last()
                .and_then(|context| {
                    context
                        .available
                        .values()
                        .find(|capture| capture.binding == *binding)
                })
                .filter(|capture| !capture.mutable)
                .map(|capture| (capture.binding, capture.ty)),
            hir::ExprKind::ConstructorParam(parameter) => self
                .constructor_params_in_scope
                .values()
                .find(|&&(id, _, _)| id == *parameter)
                .map(|&(_, ty, binding)| (binding, ty)),
            hir::ExprKind::Upcast(source)
            | hir::ExprKind::Box(source)
            | hir::ExprKind::Unbox(source) => self.flow_source_binding(source),
            _ => None,
        }?;
        // A function view creates an adapter, not an alias of the stored value.
        (!matches!(self.types[value.ty], Type::Function(_)) || value.ty == source.1)
            .then_some(source)
    }

    pub(crate) fn record_flow_value(
        &mut self,
        facts: &mut SmartCastFacts,
        local: hir::LocalId,
        value: &hir::Expr,
    ) {
        let binding = self.locals[local].binding;
        let boolean = if self.types_equal(value.ty, self.boolean) {
            Some(BooleanFacts {
                yes: self
                    .refine_flow_condition(facts.clone(), value, true)
                    .map(|facts| facts.types),
                no: self
                    .refine_flow_condition(facts.clone(), value, false)
                    .map(|facts| facts.types),
            })
        } else {
            None
        };
        let source = match &value.kind {
            hir::ExprKind::Local(_)
            | hir::ExprKind::Capture(_)
            | hir::ExprKind::ConstructorParam(_)
            | hir::ExprKind::Upcast(_) => {
                self.flow_source_binding(value).map(|(binding, _)| binding)
            }
            _ => None,
        };
        facts.forget(binding);
        if let Some(source) = source {
            facts.copy_binding(binding, source);
        }
        if let Some(boolean) = boolean {
            facts.booleans.insert(binding, boolean);
        }
    }
}
