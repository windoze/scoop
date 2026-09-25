use super::*;

resource_node!(DecodedDefaultExpressionKindV1, node, children, {
    match node {
        Self::StringLiteral { value, owner } => {
            children.push(owner)?;
            children.push(value)?;
        }
        Self::IntegerLiteral(field_0) => {
            children.push(field_0)?;
        }
        Self::BooleanLiteral(field_0) => {
            children.push(field_0)?;
        }
        Self::UnitLiteral => return Ok(()),
        Self::TupleLiteral(field_0) => {
            children.push(field_0)?;
        }
        Self::StructInit {
            constructor,
            arguments,
        } => {
            children.push(arguments)?;
            children.push(constructor)?;
        }
        Self::StructConstruct { owner_type, fields } => {
            children.push(fields)?;
            children.push(owner_type)?;
        }
        Self::ClassInit {
            constructor,
            arguments,
        } => {
            children.push(arguments)?;
            children.push(constructor)?;
        }
        Self::VariantConstruct { variant, arguments } => {
            children.push(arguments)?;
            children.push(variant)?;
        }
        Self::VariantTest { operand, variant } => {
            children.push(variant)?;
            children.push(operand)?;
        }
        Self::VariantPayloadProject { operand, field } => {
            children.push(field)?;
            children.push(operand)?;
        }
        Self::Local(field_0) => {
            children.local_index(*field_0)?;
            children.push(field_0)?;
        }
        Self::GlobalRead(field_0) => {
            children.push(field_0)?;
        }
        Self::SingletonValue(field_0) => {
            children.push(field_0)?;
        }
        Self::Lambda(field_0) => {
            children.push(field_0)?;
        }
        Self::AnonymousFunction(field_0) => {
            children.push(field_0)?;
        }
        Self::CallableReference(field_0) => {
            children.push(field_0)?;
        }
        Self::FunctionCoercion {
            source,
            source_function_type,
            target_function_type,
        } => {
            children.push(target_function_type)?;
            children.push(source_function_type)?;
            children.push(source)?;
        }
        Self::PtrFromNonZeroULong(field_0) => {
            children.push(field_0)?;
        }
        Self::PtrToULong(field_0) => {
            children.push(field_0)?;
        }
        Self::PtrCast(field_0) => {
            children.push(field_0)?;
        }
        Self::PtrLoad { pointer, offset } => {
            children.push(offset)?;
            children.push(pointer)?;
        }
        Self::PtrStore {
            pointer,
            offset,
            value,
        } => {
            children.push(value)?;
            children.push(offset)?;
            children.push(pointer)?;
        }
        Self::PtrOffset {
            pointer,
            offset,
            subtract,
        } => {
            children.push(subtract)?;
            children.push(offset)?;
            children.push(pointer)?;
        }
        Self::AddressOf(field_0) => {
            children.push(field_0)?;
        }
        Self::SizeOf(field_0) => {
            children.push(field_0)?;
        }
        Self::AlignOf(field_0) => {
            children.push(field_0)?;
        }
        Self::FunctionAddress(field_0) => {
            children.push(field_0)?;
        }
        Self::ForeignCallbackRegister {
            registration,
            closure,
        } => {
            children.push(closure)?;
            children.push(registration)?;
        }
        Self::ForeignCallbackOperation {
            operation,
            callback,
        } => {
            children.push(callback)?;
            children.push(operation)?;
        }
        Self::FieldAccess { receiver, field } => {
            children.push(field)?;
            children.push(receiver)?;
        }
        Self::MethodCall {
            receiver,
            callee,
            arguments,
        } => {
            children.push(arguments)?;
            children.push(callee)?;
            children.push(receiver)?;
        }
        Self::DirectSuperMethodCall {
            receiver,
            callee,
            arguments,
        } => {
            children.push(arguments)?;
            children.push(callee)?;
            children.push(receiver)?;
        }
        Self::Box(field_0) => {
            children.push(field_0)?;
        }
        Self::Unbox(field_0) => {
            children.push(field_0)?;
        }
        Self::IsInstance {
            operand,
            checked_type,
        } => {
            children.push(checked_type)?;
            children.push(operand)?;
        }
        Self::Cast {
            operand,
            checked_type,
            optional,
        } => {
            children.push(optional)?;
            children.push(checked_type)?;
            children.push(operand)?;
        }
        Self::ArrayLiteral(field_0) => {
            children.push(field_0)?;
        }
        Self::ArrayAssembly(field_0) => {
            children.push(field_0)?;
        }
        Self::Index {
            access,
            receiver,
            index,
        } => {
            children.push(index)?;
            children.push(receiver)?;
            children.push(access)?;
        }
        Self::ArraySet {
            access,
            receiver,
            index,
            value,
        } => {
            children.push(value)?;
            children.push(index)?;
            children.push(receiver)?;
            children.push(access)?;
        }
        Self::ArrayLen(field_0) => {
            children.push(field_0)?;
        }
        Self::ArrayClone(field_0) => {
            children.push(field_0)?;
        }
        Self::Call { callee, arguments } => {
            children.push(arguments)?;
            children.push(callee)?;
        }
        Self::LocalFunctionCall {
            declaration,
            callee,
            captures,
            arguments,
        } => {
            children.push(arguments)?;
            children.push(captures)?;
            children.push(callee)?;
            children.push(declaration)?;
        }
        Self::CallableCall {
            callee,
            function_type,
            arguments,
        } => {
            children.push(arguments)?;
            children.push(function_type)?;
            children.push(callee)?;
        }
        Self::PrimitiveBinary { kind, lhs, rhs } => {
            children.push(rhs)?;
            children.push(lhs)?;
            children.push(kind)?;
        }
        Self::PrimitiveUnary { kind, operand } => {
            children.push(operand)?;
            children.push(kind)?;
        }
        Self::IntegerOperation {
            operation,
            arguments,
        } => {
            children.push(arguments)?;
            children.push(operation)?;
        }
        Self::IntegerConversion {
            source_kind,
            target_kind,
            operand,
        } => {
            children.push(operand)?;
            children.push(target_kind)?;
            children.push(source_kind)?;
        }
        Self::Binary { operator, lhs, rhs } => {
            children.push(rhs)?;
            children.push(lhs)?;
            children.push(operator)?;
        }
        Self::Unary { operator, operand } => {
            children.push(operand)?;
            children.push(operator)?;
        }
        Self::SomeWrap(field_0) => {
            children.push(field_0)?;
        }
        Self::NoneLiteral => return Ok(()),
        Self::IsSome(field_0) => {
            children.push(field_0)?;
        }
        Self::Unwrap {
            operand,
            trap_on_none,
        } => {
            children.push(trap_on_none)?;
            children.push(operand)?;
        }
    }
    Ok(())
});
