use super::*;

impl<'a> Traversal<'a> {
    pub(super) fn expression(&mut self, expression: &'a Expr) -> Result<(), StructureError> {
        match &expression.kind {
            ExprKind::TupleLiteral(values)
            | ExprKind::ArrayLiteral(values)
            | ExprKind::StructInit { args: values, .. }
            | ExprKind::StructConstruct { fields: values, .. }
            | ExprKind::StructConstructorCall { args: values, .. }
            | ExprKind::ClassNew { args: values, .. }
            | ExprKind::VariantConstruct { args: values, .. }
            | ExprKind::Call { args: values, .. }
            | ExprKind::ImportedGenericCall { args: values, .. }
            | ExprKind::ImportedDependencyCall { args: values, .. } => self.expressions(values),
            ExprKind::ClassInitializerCall { receiver, args, .. }
            | ExprKind::MethodCall { receiver, args, .. }
            | ExprKind::DirectSuperMethodCall { receiver, args, .. }
            | ExprKind::CallableCall {
                callee: receiver,
                args,
                ..
            } => {
                self.expressions(args)?;
                self.push(Item::Expression(receiver))
            }
            ExprKind::LocalFunctionCall { captures, args, .. } => {
                self.expressions(args)?;
                self.expressions(captures)
            }
            ExprKind::VariantTest { operand, .. }
            | ExprKind::VariantPayloadProject { operand, .. }
            | ExprKind::FunctionCoercion {
                source: operand, ..
            }
            | ExprKind::PtrFromNonZeroULong(operand)
            | ExprKind::PtrToULong(operand)
            | ExprKind::PtrCast(operand)
            | ExprKind::Box(operand)
            | ExprKind::Unbox(operand)
            | ExprKind::ReferenceUpcast(operand)
            | ExprKind::IsInstance { operand, .. }
            | ExprKind::Cast { operand, .. }
            | ExprKind::ArrayLen(operand)
            | ExprKind::ArrayClone(operand)
            | ExprKind::PrimitiveUnary { operand, .. }
            | ExprKind::IntegerConversion { operand, .. }
            | ExprKind::Unary { operand, .. }
            | ExprKind::SomeWrap(operand)
            | ExprKind::IsSome(operand)
            | ExprKind::Unwrap { operand, .. }
            | ExprKind::ForeignCallbackRegister {
                closure: operand, ..
            }
            | ExprKind::ForeignCallbackOperation {
                callback: operand, ..
            }
            | ExprKind::FieldAccess {
                receiver: operand, ..
            } => self.push(Item::Expression(operand)),
            ExprKind::PrimitiveBinary { lhs, rhs, .. }
            | ExprKind::Binary { lhs, rhs, .. }
            | ExprKind::PtrOffset {
                pointer: lhs,
                offset: rhs,
                ..
            }
            | ExprKind::Index {
                receiver: lhs,
                index: rhs,
                ..
            } => self.pair(lhs, rhs),
            ExprKind::IntegerOperation { arguments, .. } => match arguments {
                HirIntegerOperationArguments::Unary(value) => self.push(Item::Expression(value)),
                HirIntegerOperationArguments::Binary { lhs, rhs } => self.pair(lhs, rhs),
            },
            ExprKind::PtrLoad { pointer, offset } => {
                if let Some(offset) = offset {
                    self.push(Item::Expression(offset))?;
                }
                self.push(Item::Expression(pointer))
            }
            ExprKind::PtrStore {
                pointer,
                offset,
                value,
            } => {
                self.push(Item::Expression(value))?;
                if let Some(offset) = offset {
                    self.push(Item::Expression(offset))?;
                }
                self.push(Item::Expression(pointer))
            }
            ExprKind::ArraySet {
                receiver,
                index,
                value,
                ..
            } => {
                self.push(Item::Expression(value))?;
                self.pair(receiver, index)
            }
            ExprKind::ArrayAssembly(assembly) => {
                for part in assembly.parts.iter().rev() {
                    let (ArrayAssemblyPart::Element(value) | ArrayAssemblyPart::CopyArray(value)) =
                        part;
                    self.push(Item::Expression(value))?;
                }
                Ok(())
            }
            ExprKind::Lambda(id) => {
                let lambda = arena_get(&self.module.lambdas, *id)
                    .ok_or(StructureError::MissingLambda(*id))?;
                self.captures(&lambda.captures)
            }
            ExprKind::AnonymousFunction(id) => {
                let function = arena_get(&self.module.anonymous_functions, *id)
                    .ok_or(StructureError::MissingAnonymousFunction(*id))?;
                self.captures(&function.captures)
            }
            ExprKind::CallableReference(id) => {
                let reference = arena_get(&self.module.callable_references, *id)
                    .ok_or(StructureError::MissingCallableReference(*id))?;
                self.captures(&reference.captures)?;
                match &reference.target {
                    CallableReferenceTarget::BoundMember { receiver, .. }
                    | CallableReferenceTarget::BoundExtension { receiver, .. }
                    | CallableReferenceTarget::BoundIntrinsic { receiver, .. } => {
                        self.push(Item::Expression(receiver))
                    }
                    CallableReferenceTarget::Named(_) | CallableReferenceTarget::Local { .. } => {
                        Ok(())
                    }
                }
            }
            ExprKind::StringLiteral { .. }
            | ExprKind::IntegerLiteral(_)
            | ExprKind::BoolLiteral(_)
            | ExprKind::UnitLiteral
            | ExprKind::ConstructorReceiver
            | ExprKind::ConstructorParam(_)
            | ExprKind::Local(_)
            | ExprKind::GlobalRead(_)
            | ExprKind::SingletonValue(_)
            | ExprKind::ImportedSingletonValue(_)
            | ExprKind::Capture(_)
            | ExprKind::AddressOf(_)
            | ExprKind::SizeOf(_)
            | ExprKind::AlignOf(_)
            | ExprKind::FunctionAddress(_)
            | ExprKind::NoneLiteral => Ok(()),
        }
    }

    fn pair(&mut self, first: &'a Expr, second: &'a Expr) -> Result<(), StructureError> {
        self.push(Item::Expression(second))?;
        self.push(Item::Expression(first))
    }
}

fn arena_get<T>(arena: &Arena<T>, id: Idx<T>) -> Option<&T> {
    ((id.into_raw().into_u32() as usize) < arena.len()).then(|| &arena[id])
}
