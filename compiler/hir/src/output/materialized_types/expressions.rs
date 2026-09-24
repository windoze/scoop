use super::*;

impl Collector<'_> {
    pub(super) fn expression(
        &mut self,
        expression: &Expr,
        meter: &mut BudgetMeter,
    ) -> Result<(), MaterializedTypeClosureError> {
        self.add(expression.ty, 1, meter)?;
        match &expression.kind {
            ExprKind::SizeOf(ty)
            | ExprKind::AlignOf(ty)
            | ExprKind::IsInstance { check_ty: ty, .. } => self.add(*ty, 1, meter),
            ExprKind::CallableCall { function_type, .. } => {
                self.function_type(*function_type, 1, meter)
            }
            ExprKind::ArrayAssembly(assembly) => self.add(assembly.element_type, 1, meter),
            // Child expressions (including closure captures) are visited by
            // the shared executable traversal. Their types remain explicit.
            ExprKind::StringLiteral { .. }
            | ExprKind::IntegerLiteral(_)
            | ExprKind::BoolLiteral(_)
            | ExprKind::UnitLiteral
            | ExprKind::TupleLiteral(_)
            | ExprKind::StructInit { .. }
            | ExprKind::StructConstruct { .. }
            | ExprKind::StructConstructorCall { .. }
            | ExprKind::ClassNew { .. }
            | ExprKind::ClassInitializerCall { .. }
            | ExprKind::ConstructorReceiver
            | ExprKind::ConstructorParam(_)
            | ExprKind::VariantConstruct { .. }
            | ExprKind::VariantTest { .. }
            | ExprKind::VariantPayloadProject { .. }
            | ExprKind::Local(_)
            | ExprKind::GlobalRead(_)
            | ExprKind::SingletonValue(_)
            | ExprKind::Capture(_)
            | ExprKind::Lambda(_)
            | ExprKind::AnonymousFunction(_)
            | ExprKind::CallableReference(_)
            | ExprKind::FunctionCoercion { .. }
            | ExprKind::PtrFromNonZeroULong(_)
            | ExprKind::PtrToULong(_)
            | ExprKind::PtrCast(_)
            | ExprKind::PtrLoad { .. }
            | ExprKind::PtrStore { .. }
            | ExprKind::PtrOffset { .. }
            | ExprKind::AddressOf(_)
            | ExprKind::FunctionAddress(_)
            | ExprKind::ForeignCallbackRegister { .. }
            | ExprKind::ForeignCallbackOperation { .. }
            | ExprKind::FieldAccess { .. }
            | ExprKind::MethodCall { .. }
            | ExprKind::DirectSuperMethodCall { .. }
            | ExprKind::Box(_)
            | ExprKind::Unbox(_)
            | ExprKind::Cast { .. }
            | ExprKind::ArrayLiteral(_)
            | ExprKind::Index { .. }
            | ExprKind::ArraySet { .. }
            | ExprKind::ArrayLen(_)
            | ExprKind::ArrayClone(_)
            | ExprKind::Call { .. }
            | ExprKind::ImportedDependencyCall { .. }
            | ExprKind::LocalFunctionCall { .. }
            | ExprKind::PrimitiveBinary { .. }
            | ExprKind::PrimitiveUnary { .. }
            | ExprKind::IntegerOperation { .. }
            | ExprKind::IntegerConversion { .. }
            | ExprKind::Binary { .. }
            | ExprKind::Unary { .. }
            | ExprKind::SomeWrap(_)
            | ExprKind::NoneLiteral
            | ExprKind::IsSome(_)
            | ExprKind::Unwrap { .. } => Ok(()),
        }
    }
}
