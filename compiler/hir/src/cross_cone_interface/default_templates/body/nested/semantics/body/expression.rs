use crate::{
    DefaultArrayAssemblyPartV1, DefaultCallableReferenceTargetV1, DefaultExpressionKindV1,
    DefaultExpressionV1, DefaultIntegerArgumentsV1,
};

use super::BodyNode;
use crate::cross_cone_interface::default_templates::body::nested::semantics::{
    DefaultBodyNestedAuthority, DefaultNestedCallableAbiValidationError,
    DefaultNestedCallableLocalUseV1, Validator,
};

impl<A, E> Validator<'_, A, E>
where
    A: DefaultBodyNestedAuthority<E>,
{
    pub(super) fn process_expression<'body>(
        &mut self,
        expression: &'body DefaultExpressionV1,
        pending: &mut Vec<BodyNode<'body>>,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        match expression.kind() {
            DefaultExpressionKindV1::StringLiteral { .. }
            | DefaultExpressionKindV1::IntegerLiteral(_)
            | DefaultExpressionKindV1::BooleanLiteral(_)
            | DefaultExpressionKindV1::UnitLiteral
            | DefaultExpressionKindV1::Local(_)
            | DefaultExpressionKindV1::GlobalRead(_)
            | DefaultExpressionKindV1::SingletonValue(_)
            | DefaultExpressionKindV1::AddressOf(_)
            | DefaultExpressionKindV1::SizeOf(_)
            | DefaultExpressionKindV1::AlignOf(_)
            | DefaultExpressionKindV1::FunctionAddress(_)
            | DefaultExpressionKindV1::NoneLiteral => Ok(()),
            DefaultExpressionKindV1::TupleLiteral(elements)
            | DefaultExpressionKindV1::ArrayLiteral(elements) => {
                self.push_expressions(pending, elements)
            }
            DefaultExpressionKindV1::StructInit { arguments, .. }
            | DefaultExpressionKindV1::ClassInit { arguments, .. }
            | DefaultExpressionKindV1::VariantConstruct { arguments, .. } => {
                self.push_expressions(pending, arguments)
            }
            DefaultExpressionKindV1::StructConstruct { fields, .. } => {
                self.push_expressions(pending, fields)
            }
            DefaultExpressionKindV1::VariantTest { operand, .. }
            | DefaultExpressionKindV1::VariantPayloadProject { operand, .. }
            | DefaultExpressionKindV1::FunctionCoercion {
                source: operand, ..
            }
            | DefaultExpressionKindV1::PtrFromNonZeroULong(operand)
            | DefaultExpressionKindV1::PtrToULong(operand)
            | DefaultExpressionKindV1::PtrCast(operand)
            | DefaultExpressionKindV1::Box(operand)
            | DefaultExpressionKindV1::Unbox(operand)
            | DefaultExpressionKindV1::ArrayLen(operand)
            | DefaultExpressionKindV1::ArrayClone(operand)
            | DefaultExpressionKindV1::PrimitiveUnary { operand, .. }
            | DefaultExpressionKindV1::Unary { operand, .. }
            | DefaultExpressionKindV1::SomeWrap(operand)
            | DefaultExpressionKindV1::IsSome(operand)
            | DefaultExpressionKindV1::Unwrap { operand, .. }
            | DefaultExpressionKindV1::Cast { operand, .. }
            | DefaultExpressionKindV1::IsInstance { operand, .. }
            | DefaultExpressionKindV1::IntegerConversion { operand, .. } => {
                self.push_child(pending, BodyNode::Expression(operand))
            }
            DefaultExpressionKindV1::Lambda(lambda) => {
                self.push_child(pending, BodyNode::Lambda(lambda))
            }
            DefaultExpressionKindV1::AnonymousFunction(function) => {
                self.push_child(pending, BodyNode::AnonymousFunction(function))
            }
            DefaultExpressionKindV1::CallableReference(reference) => {
                self.push_child(pending, BodyNode::CallableReference(reference))
            }
            DefaultExpressionKindV1::PtrLoad { pointer, offset } => {
                self.push_optional_expression(pending, offset.as_ref())?;
                self.push_child(pending, BodyNode::Expression(pointer))
            }
            DefaultExpressionKindV1::PtrStore {
                pointer,
                offset,
                value,
            } => {
                self.push_child(pending, BodyNode::Expression(value))?;
                self.push_optional_expression(pending, offset.as_ref())?;
                self.push_child(pending, BodyNode::Expression(pointer))
            }
            DefaultExpressionKindV1::PtrOffset {
                pointer, offset, ..
            }
            | DefaultExpressionKindV1::Index {
                receiver: pointer,
                index: offset,
                ..
            } => {
                self.push_child(pending, BodyNode::Expression(offset))?;
                self.push_child(pending, BodyNode::Expression(pointer))
            }
            DefaultExpressionKindV1::ForeignCallbackRegister { closure, .. } => {
                self.push_child(pending, BodyNode::Expression(closure))
            }
            DefaultExpressionKindV1::ForeignCallbackOperation { callback, .. } => {
                self.push_child(pending, BodyNode::Expression(callback))
            }
            DefaultExpressionKindV1::FieldAccess { receiver, .. } => {
                self.push_child(pending, BodyNode::Expression(receiver))
            }
            DefaultExpressionKindV1::MethodCall {
                receiver,
                arguments,
                ..
            }
            | DefaultExpressionKindV1::DirectSuperMethodCall {
                receiver,
                arguments,
                ..
            } => {
                self.push_expressions(pending, arguments)?;
                self.push_child(pending, BodyNode::Expression(receiver))
            }
            DefaultExpressionKindV1::ArrayAssembly(assembly) => {
                for part in assembly.parts().iter().rev() {
                    let expression = match part {
                        DefaultArrayAssemblyPartV1::Element(expression)
                        | DefaultArrayAssemblyPartV1::CopyArray(expression) => expression,
                    };
                    self.push_child(pending, BodyNode::Expression(expression))?;
                }
                Ok(())
            }
            DefaultExpressionKindV1::ArraySet {
                receiver,
                index,
                value,
                ..
            } => {
                self.push_child(pending, BodyNode::Expression(value))?;
                self.push_child(pending, BodyNode::Expression(index))?;
                self.push_child(pending, BodyNode::Expression(receiver))
            }
            DefaultExpressionKindV1::Call { arguments, .. } => {
                self.push_expressions(pending, arguments)
            }
            DefaultExpressionKindV1::LocalFunctionCall {
                declaration,
                captures,
                arguments,
                ..
            } => {
                self.record_local_use(*declaration, DefaultNestedCallableLocalUseV1::DirectCall)?;
                self.push_expressions(pending, arguments)?;
                self.push_expressions(pending, captures)
            }
            DefaultExpressionKindV1::CallableCall {
                callee, arguments, ..
            } => {
                self.push_expressions(pending, arguments)?;
                self.push_child(pending, BodyNode::Expression(callee))
            }
            DefaultExpressionKindV1::PrimitiveBinary { lhs, rhs, .. }
            | DefaultExpressionKindV1::Binary { lhs, rhs, .. } => {
                self.push_child(pending, BodyNode::Expression(rhs))?;
                self.push_child(pending, BodyNode::Expression(lhs))
            }
            DefaultExpressionKindV1::IntegerOperation { arguments, .. } => {
                self.push_integer_arguments(pending, arguments)
            }
        }
    }

    pub(super) fn process_callable_reference<'body>(
        &mut self,
        reference: &'body crate::DefaultCallableReferenceV1,
        pending: &mut Vec<BodyNode<'body>>,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        match reference.target() {
            DefaultCallableReferenceTargetV1::Named(_) => Ok(()),
            DefaultCallableReferenceTargetV1::Local { declaration, .. } => self.record_local_use(
                *declaration,
                DefaultNestedCallableLocalUseV1::CallableReference,
            ),
            DefaultCallableReferenceTargetV1::BoundMember { receiver, .. }
            | DefaultCallableReferenceTargetV1::BoundExtension { receiver, .. } => {
                self.push_child(pending, BodyNode::Expression(receiver))
            }
        }
    }

    fn push_optional_expression<'body>(
        &mut self,
        pending: &mut Vec<BodyNode<'body>>,
        expression: Option<&'body DefaultExpressionV1>,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        match expression {
            Some(expression) => self.push_child(pending, BodyNode::Expression(expression)),
            None => Ok(()),
        }
    }

    pub(super) fn push_expressions<'body>(
        &mut self,
        pending: &mut Vec<BodyNode<'body>>,
        expressions: &'body [DefaultExpressionV1],
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        for expression in expressions.iter().rev() {
            self.push_child(pending, BodyNode::Expression(expression))?;
        }
        Ok(())
    }

    fn push_integer_arguments<'body>(
        &mut self,
        pending: &mut Vec<BodyNode<'body>>,
        arguments: &'body DefaultIntegerArgumentsV1,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        match arguments {
            DefaultIntegerArgumentsV1::Unary(operand) => {
                self.push_child(pending, BodyNode::Expression(operand))
            }
            DefaultIntegerArgumentsV1::Binary { lhs, rhs } => {
                self.push_child(pending, BodyNode::Expression(rhs))?;
                self.push_child(pending, BodyNode::Expression(lhs))
            }
        }
    }
}
