use super::*;

impl CfgLowerer<'_> {
    pub(super) fn lower_pair(
        &mut self,
        lhs: &smir::Expr,
        rhs: &smir::Expr,
        span: Span,
    ) -> Option<(mir::Expr, mir::Expr)> {
        let mut lhs = self.lower_expr(lhs, span)?;
        if emits_cfg(rhs) {
            lhs = self.save_operand(lhs, span);
        }
        Some((lhs, self.lower_expr(rhs, span)?))
    }

    pub(super) fn lower_operands<'e>(
        &mut self,
        operands: impl IntoIterator<Item = &'e smir::Expr>,
        span: Span,
    ) -> Option<Vec<mir::Expr>> {
        let operands: Vec<_> = operands.into_iter().collect();
        let last_effect = operands.iter().rposition(|operand| emits_cfg(operand));
        operands
            .into_iter()
            .enumerate()
            .map(|(index, operand)| {
                let value = self.lower_expr(operand, span)?;
                Some(if last_effect.is_some_and(|last| index < last) {
                    self.save_operand(value, span)
                } else {
                    value
                })
            })
            .collect()
    }

    fn save_operand(&mut self, value: mir::Expr, span: Span) -> mir::Expr {
        use mir::ExprKind;
        let stable = match &value.kind {
            ExprKind::Local(local) => !self.locals[*local].mutable,
            ExprKind::StringConst(_)
            | ExprKind::IntegerLiteral(_)
            | ExprKind::MachineScalarLiteral(_)
            | ExprKind::FloatLiteral(_)
            | ExprKind::CharLiteral(_)
            | ExprKind::BoolLiteral(_)
            | ExprKind::UnitLiteral
            | ExprKind::AddressOf { .. }
            | ExprKind::GlobalAddress { .. }
            | ExprKind::InitializationUnitAddress(_)
            | ExprKind::FunctionAddress { .. }
            | ExprKind::SizeOf(_)
            | ExprKind::AlignOf(_) => true,
            _ => false,
        };
        if stable {
            return value;
        }
        let local = self.new_hidden(
            "operand",
            StructuralDefinitionSiteRole::SyntheticValue,
            SyntheticLocalRole::Temporary,
            value.ty.clone(),
        );
        let result = mir::Expr::new(value.ty.clone(), mir::ExprKind::Local(local));
        self.push(mir::StatementKind::ValDecl { local, init: value }, span);
        result
    }
}

/// Only operations lifted out of the expression tree require saving earlier
/// siblings here. Call-free operations still execute in tree evaluation order.
fn emits_cfg(expr: &smir::Expr) -> bool {
    use smir::ExprKind;
    match &expr.kind {
        ExprKind::Call(_)
        | ExprKind::ClassNew { .. }
        | ExprKind::ShortCircuit { .. }
        | ExprKind::Unreachable
        | ExprKind::Diverging { .. } => true,
        ExprKind::MaybeUninit { operation, .. } => {
            operation.operand().is_some_and(|value| emits_cfg(value))
        }
        ExprKind::Context(operation) => operation.operand().is_some_and(emits_cfg),
        ExprKind::DataBorrow(operation) => emits_cfg(&operation.operand),
        ExprKind::Atomic(atomic) => atomic.operands().any(emits_cfg),
        ExprKind::TupleLiteral(values)
        | ExprKind::StructInit { args: values, .. }
        | ExprKind::StructConstruct { fields: values, .. }
        | ExprKind::ArrayLiteral {
            elements: values, ..
        }
        | ExprKind::VariantConstruct { fields: values, .. } => values.iter().any(emits_cfg),
        ExprKind::ClosureAlloc { captures, .. } => {
            captures.iter().any(|capture| emits_cfg(&capture.value))
        }
        ExprKind::ArrayAssembly { parts, .. } => parts.iter().any(|part| match part {
            smir::ArrayAssemblyPart::Element(value) | smir::ArrayAssemblyPart::CopyArray(value) => {
                emits_cfg(value)
            }
        }),
        ExprKind::ClosureCapture {
            closure: operand, ..
        }
        | ExprKind::PtrFromNonZeroULong { operand, .. }
        | ExprKind::PtrToULong(operand)
        | ExprKind::PtrCast { operand, .. }
        | ExprKind::CharCode(operand)
        | ExprKind::CharFromCodeUnchecked(operand)
        | ExprKind::FloatUnary { operand, .. }
        | ExprKind::FloatConversion { operand, .. }
        | ExprKind::ForeignCallbackRegister {
            closure: operand, ..
        }
        | ExprKind::ForeignCallbackOperation {
            callback: operand, ..
        }
        | ExprKind::Retype { operand, .. }
        | ExprKind::FieldAccess {
            receiver: operand, ..
        }
        | ExprKind::Box(operand)
        | ExprKind::Unbox(operand)
        | ExprKind::IsInstance { operand, .. }
        | ExprKind::ArrayAllocate { count: operand, .. }
        | ExprKind::ArrayLen { operand, .. }
        | ExprKind::ArrayClone { operand, .. }
        | ExprKind::AtomicNew(operand)
        | ExprKind::Unary { operand, .. }
        | ExprKind::IntegerUnary { operand, .. }
        | ExprKind::IntegerConversion { operand, .. }
        | ExprKind::VariantTest { operand, .. }
        | ExprKind::VariantPayloadProject { operand, .. } => emits_cfg(operand),
        ExprKind::FloatBinary { lhs, rhs, .. }
        | ExprKind::Binary { lhs, rhs, .. }
        | ExprKind::IntegerBinary { lhs, rhs, .. }
        | ExprKind::SafeIntegerDivRem { lhs, rhs, .. }
        | ExprKind::IntegerCompare { lhs, rhs, .. }
        | ExprKind::IntegerCompareTo { lhs, rhs, .. }
        | ExprKind::IntegerShift {
            value: lhs,
            count: rhs,
            ..
        }
        | ExprKind::PtrOffset {
            pointer: lhs,
            offset: rhs,
            ..
        }
        | ExprKind::ArrayGet {
            array: lhs,
            index: rhs,
            ..
        } => emits_cfg(lhs) || emits_cfg(rhs),
        ExprKind::PtrLoad {
            pointer, offset, ..
        } => emits_cfg(pointer) || offset.as_deref().is_some_and(emits_cfg),
        ExprKind::PtrStore {
            pointer,
            offset,
            value,
            ..
        } => emits_cfg(pointer) || offset.as_deref().is_some_and(emits_cfg) || emits_cfg(value),
        ExprKind::StringConst(_)
        | ExprKind::IntegerLiteral(_)
        | ExprKind::MachineScalarLiteral(_)
        | ExprKind::CharLiteral(_)
        | ExprKind::FloatLiteral(_)
        | ExprKind::BoolLiteral(_)
        | ExprKind::UnitLiteral
        | ExprKind::Local(_)
        | ExprKind::GlobalRead(_)
        | ExprKind::InitializationUnitAddress(_)
        | ExprKind::AddressOf { .. }
        | ExprKind::GlobalAddress { .. }
        | ExprKind::SizeOf(_)
        | ExprKind::AlignOf(_)
        | ExprKind::FunctionAddress { .. }
        | ExprKind::ReleaseFieldLoad { .. } => false,
    }
}
