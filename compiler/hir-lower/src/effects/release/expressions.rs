use super::*;

impl Lowerer {
    pub(super) fn release_expression(
        &self,
        expression: &hir::Expr,
        values: &ReleaseValueFacts,
        facts: &mut BodyFacts,
    ) {
        use hir::ExprKind as E;
        facts.require(values.requirements(self, expression.ty));
        if facts.requirements.is_none() {
            facts.violation.get_or_insert(expression.span);
        }
        match &expression.kind {
            E::ContextLookup(_) => {
                facts.requirements = None;
                facts.violation.get_or_insert(expression.span);
            }
            E::IntegerLiteral(_)
            | E::CharLiteral(_)
            | E::FloatLiteral(_)
            | E::BoolLiteral(_)
            | E::UnitLiteral
            | E::Local(_)
            | E::ConstructorParam(_)
            | E::InitializingStructFieldAccess { .. }
            | E::ReleaseFieldLoad(_)
            | E::NoneLiteral
            | E::SizeOf(_)
            | E::AlignOf(_) => {}
            E::GlobalRead(global) => {
                if !self.release_global(*global) {
                    facts.requirements = None;
                }
            }
            E::TupleLiteral(elements)
            | E::StructConstruct {
                fields: elements, ..
            }
            | E::VariantConstruct { args: elements, .. } => {
                for element in elements {
                    self.release_expression(element, values, facts);
                }
            }
            E::StructInit { constructor, args } => {
                self.release_constructor_call(*constructor, expression.span, values, facts);
                for argument in args {
                    self.release_expression(argument, values, facts);
                }
            }
            E::Call { callee, args, .. } => {
                self.release_call(*callee, expression.span, values, facts);
                for argument in args {
                    self.release_expression(argument, values, facts);
                }
            }
            E::MethodCall {
                receiver,
                callee,
                args,
            }
            | E::DirectSuperMethodCall {
                receiver,
                callee,
                args,
            } => {
                if let hir::MethodCallee::Callable(callee) = callee {
                    self.release_call(*callee, expression.span, values, facts);
                } else {
                    facts.requirements = None;
                }
                self.release_expression(receiver, values, facts);
                for argument in args {
                    self.release_expression(argument, values, facts);
                }
            }
            E::FieldAccess { receiver, field } => {
                if matches!(field, hir::FieldRef::ClassField { .. }) {
                    facts.requirements = None;
                }
                self.release_expression(receiver, values, facts);
            }
            E::VariantTest { operand, .. }
            | E::VariantPayloadProject { operand, .. }
            | E::PtrFromNonZeroULong(operand)
            | E::CharCode(operand)
            | E::CharFromCodeUnchecked(operand)
            | E::PtrToULong(operand)
            | E::PtrCast(operand)
            | E::IntegerConversion { operand, .. }
            | E::FloatUnary { operand, .. }
            | E::FloatConversion { operand, .. }
            | E::Unary { operand, .. }
            | E::PrimitiveUnary { operand, .. }
            | E::SomeWrap(operand)
            | E::IsSome(operand) => {
                self.release_expression(operand, values, facts);
            }
            E::Binary { lhs, rhs, .. }
            | E::FloatBinary { lhs, rhs, .. }
            | E::PrimitiveBinary { lhs, rhs, .. } => {
                self.release_expression(lhs, values, facts);
                self.release_expression(rhs, values, facts);
            }
            E::IntegerOperation {
                operation,
                arguments,
            } => {
                if matches!(operation, hir::IntegerOperation::Managed { .. }) {
                    facts.requirements = None;
                }
                match arguments {
                    hir::HirIntegerOperationArguments::Unary(operand) => {
                        self.release_expression(operand, values, facts);
                    }
                    hir::HirIntegerOperationArguments::Binary { lhs, rhs } => {
                        self.release_expression(lhs, values, facts);
                        self.release_expression(rhs, values, facts);
                    }
                }
            }
            E::PtrLoad { pointer, offset } => {
                self.release_expression(pointer, values, facts);
                if let Some(offset) = offset {
                    self.release_expression(offset, values, facts);
                }
            }
            E::PtrOffset {
                pointer, offset, ..
            } => {
                self.release_expression(pointer, values, facts);
                self.release_expression(offset, values, facts);
            }
            E::PtrStore {
                pointer,
                offset,
                value,
            } => {
                self.release_expression(pointer, values, facts);
                if let Some(offset) = offset {
                    self.release_expression(offset, values, facts);
                }
                self.release_expression(value, values, facts);
            }
            E::AddressOf(place) => match place {
                hir::Place::Local(_) => {}
                hir::Place::Global(global) if self.release_global(*global) => {}
                hir::Place::ExternalGlobal {
                    source_contract, ..
                } if matches!(
                    source_contract.contract(),
                    scoop_identity::SourceNativeExternalContract::ReadOnlyData { .. }
                        | scoop_identity::SourceNativeExternalContract::MutableData { .. }
                ) => {}
                _ => facts.requirements = None,
            },
            E::Unwrap {
                operand,
                trap_on_none,
            } => {
                if *trap_on_none {
                    facts.requirements = None;
                }
                self.release_expression(operand, values, facts);
            }
            E::StringLiteral { .. }
            | E::ClassInit { .. }
            | E::ConstructorReceiver
            | E::GenericDelegateStorageRead(_)
            | E::SingletonValue(_)
            | E::Capture(_)
            | E::Lambda(_)
            | E::AnonymousFunction(_)
            | E::CallableReference(_)
            | E::FunctionCoercion { .. }
            | E::FunctionAddress(_)
            | E::ForeignCallbackRegister { .. }
            | E::ForeignCallbackOperation { .. }
            | E::InitializingClassFieldAccess { .. }
            | E::Box(_)
            | E::Unbox(_)
            | E::ReferenceUpcast(_)
            | E::IsInstance { .. }
            | E::Cast { .. }
            | E::ArrayGenerate { .. }
            | E::ArrayLiteral(_)
            | E::ArrayAssembly(_)
            | E::Index { .. }
            | E::ArraySet { .. }
            | E::ArrayLen(_)
            | E::ArrayClone(_)
            | E::CallableCall { .. } => facts.requirements = None,
        }
        if facts.requirements.is_none() {
            facts.violation.get_or_insert(expression.span);
        }
    }
}
