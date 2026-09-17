use crate::{
    CanonicalBooleanV1, DefaultArrayAssemblyPartV1, DefaultCallableReferenceTargetV1,
    DefaultCaptureV1, DefaultExpressionKindV1, DefaultExpressionV1, DefaultIntegerArgumentsV1,
    DefaultPlaceV1,
};

use super::{DefaultLocalDataFlowSiteV1, ExportDefaultLocalDataFlowValidationError, Validator};

impl<A, E> Validator<'_, A, E>
where
    A: super::DefaultLocalDataFlowSemanticAuthority<E>,
{
    pub(super) fn validate_expression(
        &mut self,
        expression: &DefaultExpressionV1,
        available: &[bool],
        reachable: bool,
        parent_depth: u64,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        let depth = self.child_depth(parent_depth)?;
        let mut pending = Vec::new();
        self.meter
            .try_reserve_collection_slots(&mut pending, 1, self.path)
            .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;
        self.enter_edge(depth)?;
        pending.push(LocalWork::Expression { expression, depth });
        self.run_local_work(&mut pending, available, reachable)
    }

    pub(super) fn validate_captures(
        &mut self,
        captures: &[DefaultCaptureV1],
        available: &[bool],
        reachable: bool,
        parent_depth: u64,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        let mut pending = Vec::new();
        self.push_captures(&mut pending, captures, parent_depth)?;
        self.run_local_work(&mut pending, available, reachable)
    }

    fn run_local_work(
        &mut self,
        pending: &mut Vec<LocalWork<'_>>,
        available: &[bool],
        reachable: bool,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        while let Some(work) = pending.pop() {
            match work {
                LocalWork::Expression { expression, depth } => {
                    self.enter_node(depth)?;
                    self.process_expression(expression, depth, pending, available, reachable)?;
                }
                LocalWork::Capture {
                    capture,
                    index,
                    depth,
                } => {
                    self.enter_node(depth)?;
                    let local_index = self.use_local(
                        capture.source(),
                        Some(capture.value_type()),
                        DefaultLocalDataFlowSiteV1::Capture { index },
                        available,
                        reachable,
                    )?;
                    self.check_local_shape(
                        local_index,
                        None,
                        Some(CanonicalBooleanV1::False),
                        DefaultLocalDataFlowSiteV1::Capture { index },
                    )?;
                }
            }
        }
        Ok(())
    }

    fn process_expression<'body>(
        &mut self,
        expression: &'body DefaultExpressionV1,
        depth: u64,
        pending: &mut Vec<LocalWork<'body>>,
        available: &[bool],
        reachable: bool,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        match expression.kind() {
            DefaultExpressionKindV1::StringLiteral { .. }
            | DefaultExpressionKindV1::IntegerLiteral(_)
            | DefaultExpressionKindV1::BooleanLiteral(_)
            | DefaultExpressionKindV1::UnitLiteral
            | DefaultExpressionKindV1::GlobalRead(_)
            | DefaultExpressionKindV1::SingletonValue(_)
            | DefaultExpressionKindV1::SizeOf(_)
            | DefaultExpressionKindV1::AlignOf(_)
            | DefaultExpressionKindV1::FunctionAddress(_)
            | DefaultExpressionKindV1::NoneLiteral => Ok(()),
            DefaultExpressionKindV1::Local(local) => {
                // Operation typing owns the principal-to-result reference retype.
                self.use_local(
                    local,
                    None,
                    DefaultLocalDataFlowSiteV1::Expression,
                    available,
                    reachable,
                )?;
                Ok(())
            }
            DefaultExpressionKindV1::TupleLiteral(elements)
            | DefaultExpressionKindV1::ArrayLiteral(elements) => {
                self.push_expressions(pending, elements, depth)
            }
            DefaultExpressionKindV1::StructInit { arguments, .. }
            | DefaultExpressionKindV1::ClassInit { arguments, .. }
            | DefaultExpressionKindV1::VariantConstruct { arguments, .. }
            | DefaultExpressionKindV1::Call { arguments, .. } => {
                self.push_expressions(pending, arguments, depth)
            }
            DefaultExpressionKindV1::StructConstruct { fields, .. } => {
                self.push_expressions(pending, fields, depth)
            }
            DefaultExpressionKindV1::VariantTest { operand, .. }
            | DefaultExpressionKindV1::VariantPayloadProject { operand, .. }
            | DefaultExpressionKindV1::FunctionCoercion {
                source: operand, ..
            }
            | DefaultExpressionKindV1::PtrFromNonZeroULong(operand)
            | DefaultExpressionKindV1::PtrToULong(operand)
            | DefaultExpressionKindV1::PtrCast(operand)
            | DefaultExpressionKindV1::ForeignCallbackRegister {
                closure: operand, ..
            }
            | DefaultExpressionKindV1::ForeignCallbackOperation {
                callback: operand, ..
            }
            | DefaultExpressionKindV1::FieldAccess {
                receiver: operand, ..
            }
            | DefaultExpressionKindV1::Box(operand)
            | DefaultExpressionKindV1::Unbox(operand)
            | DefaultExpressionKindV1::IsInstance { operand, .. }
            | DefaultExpressionKindV1::Cast { operand, .. }
            | DefaultExpressionKindV1::ArrayLen(operand)
            | DefaultExpressionKindV1::ArrayClone(operand)
            | DefaultExpressionKindV1::PrimitiveUnary { operand, .. }
            | DefaultExpressionKindV1::IntegerConversion { operand, .. }
            | DefaultExpressionKindV1::Unary { operand, .. }
            | DefaultExpressionKindV1::SomeWrap(operand)
            | DefaultExpressionKindV1::IsSome(operand)
            | DefaultExpressionKindV1::Unwrap { operand, .. } => {
                self.push_expression(pending, operand, depth)
            }
            DefaultExpressionKindV1::PtrLoad { pointer, offset } => {
                if let Some(offset) = offset.as_ref() {
                    self.push_expression(pending, offset, depth)?;
                }
                self.push_expression(pending, pointer, depth)
            }
            DefaultExpressionKindV1::PtrStore {
                pointer,
                offset,
                value,
            } => {
                self.push_expression(pending, value, depth)?;
                if let Some(offset) = offset.as_ref() {
                    self.push_expression(pending, offset, depth)?;
                }
                self.push_expression(pending, pointer, depth)
            }
            DefaultExpressionKindV1::PtrOffset {
                pointer, offset, ..
            }
            | DefaultExpressionKindV1::Index {
                receiver: pointer,
                index: offset,
                ..
            }
            | DefaultExpressionKindV1::PrimitiveBinary {
                lhs: pointer,
                rhs: offset,
                ..
            }
            | DefaultExpressionKindV1::Binary {
                lhs: pointer,
                rhs: offset,
                ..
            } => {
                self.push_expression(pending, offset, depth)?;
                self.push_expression(pending, pointer, depth)
            }
            DefaultExpressionKindV1::AddressOf(place) => {
                if let DefaultPlaceV1::Local { local } = place {
                    self.use_local(
                        local,
                        None,
                        DefaultLocalDataFlowSiteV1::AddressOf,
                        available,
                        reachable,
                    )?;
                }
                Ok(())
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
                self.push_expressions(pending, arguments, depth)?;
                self.push_expression(pending, receiver, depth)
            }
            DefaultExpressionKindV1::Lambda(lambda) => {
                self.push_captures(pending, lambda.captures(), depth)
            }
            DefaultExpressionKindV1::AnonymousFunction(function) => {
                self.push_captures(pending, function.captures(), depth)
            }
            DefaultExpressionKindV1::CallableReference(reference) => {
                self.push_captures(pending, reference.captures(), depth)?;
                match reference.target() {
                    DefaultCallableReferenceTargetV1::BoundMember { receiver, .. }
                    | DefaultCallableReferenceTargetV1::BoundExtension { receiver, .. } => {
                        self.push_expression(pending, receiver, depth)
                    }
                    DefaultCallableReferenceTargetV1::Named(_)
                    | DefaultCallableReferenceTargetV1::Local { .. } => Ok(()),
                }
            }
            DefaultExpressionKindV1::ArrayAssembly(assembly) => {
                for part in assembly.parts().iter().rev() {
                    match part {
                        DefaultArrayAssemblyPartV1::Element(value)
                        | DefaultArrayAssemblyPartV1::CopyArray(value) => {
                            self.push_expression(pending, value, depth)?;
                        }
                    }
                }
                Ok(())
            }
            DefaultExpressionKindV1::ArraySet {
                receiver,
                index,
                value,
                ..
            } => {
                self.push_expression(pending, value, depth)?;
                self.push_expression(pending, index, depth)?;
                self.push_expression(pending, receiver, depth)
            }
            DefaultExpressionKindV1::LocalFunctionCall {
                captures,
                arguments,
                ..
            } => {
                self.push_expressions(pending, arguments, depth)?;
                self.push_expressions(pending, captures, depth)
            }
            DefaultExpressionKindV1::CallableCall {
                callee, arguments, ..
            } => {
                self.push_expressions(pending, arguments, depth)?;
                self.push_expression(pending, callee, depth)
            }
            DefaultExpressionKindV1::IntegerOperation { arguments, .. } => match arguments {
                DefaultIntegerArgumentsV1::Unary(operand) => {
                    self.push_expression(pending, operand, depth)
                }
                DefaultIntegerArgumentsV1::Binary { lhs, rhs } => {
                    self.push_expression(pending, rhs, depth)?;
                    self.push_expression(pending, lhs, depth)
                }
            },
        }
    }

    fn push_expression<'body>(
        &mut self,
        pending: &mut Vec<LocalWork<'body>>,
        expression: &'body DefaultExpressionV1,
        parent_depth: u64,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        let depth = self.child_depth(parent_depth)?;
        self.meter
            .try_reserve_collection_slots(pending, 1, self.path)
            .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;
        self.enter_edge(depth)?;
        pending.push(LocalWork::Expression { expression, depth });
        Ok(())
    }

    fn push_expressions<'body>(
        &mut self,
        pending: &mut Vec<LocalWork<'body>>,
        expressions: &'body [DefaultExpressionV1],
        parent_depth: u64,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        for expression in expressions.iter().rev() {
            self.push_expression(pending, expression, parent_depth)?;
        }
        Ok(())
    }

    fn push_captures<'body>(
        &mut self,
        pending: &mut Vec<LocalWork<'body>>,
        captures: &'body [DefaultCaptureV1],
        parent_depth: u64,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        let depth = self.child_depth(parent_depth)?;
        for (index, capture) in captures.iter().enumerate().rev() {
            self.meter
                .try_reserve_collection_slots(pending, 1, self.path)
                .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;
            self.enter_edge(depth)?;
            pending.push(LocalWork::Capture {
                capture,
                index,
                depth,
            });
        }
        Ok(())
    }
}

enum LocalWork<'a> {
    Expression {
        expression: &'a DefaultExpressionV1,
        depth: u64,
    },
    Capture {
        capture: &'a DefaultCaptureV1,
        index: usize,
        depth: u64,
    },
}
