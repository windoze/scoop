use crate::{
    DefaultArrayAssemblyPartV1, DefaultArrayAssemblyV1, DefaultExpressionKindV1,
    DefaultExpressionV1, DefaultIntegerArgumentsV1,
};

use super::{BodyNode, BodyWalkMode, Validator, WorkItem};
use crate::{DefaultBodyOriginSiteV1, DefaultBodyProviderTypeSiteV1};

impl<M> Validator<'_, M>
where
    M: BodyWalkMode,
{
    pub(super) fn process_expression<'body>(
        &mut self,
        expression: &'body DefaultExpressionV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        self.process_expression_kind(
            expression.kind(),
            expression.definition_origin(),
            depth,
            pending,
        )?;
        self.push_type(
            pending,
            expression.result_type(),
            DefaultBodyProviderTypeSiteV1::ExpressionResult,
            expression.definition_origin(),
        )?;
        self.push_child(
            pending,
            depth,
            BodyNode::Origin {
                source: expression.definition_origin(),
                site: DefaultBodyOriginSiteV1::Expression,
            },
        )
    }

    fn process_expression_kind<'body>(
        &mut self,
        kind: &'body DefaultExpressionKindV1,
        definition_origin: &'body crate::ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        match kind {
            DefaultExpressionKindV1::StringLiteral { .. }
            | DefaultExpressionKindV1::IntegerLiteral(_)
            | DefaultExpressionKindV1::BooleanLiteral(_)
            | DefaultExpressionKindV1::UnitLiteral
            | DefaultExpressionKindV1::Local(_)
            | DefaultExpressionKindV1::GlobalRead(_)
            | DefaultExpressionKindV1::SingletonValue(_)
            | DefaultExpressionKindV1::AddressOf(_)
            | DefaultExpressionKindV1::FunctionAddress(_)
            | DefaultExpressionKindV1::NoneLiteral => Ok(()),
            DefaultExpressionKindV1::TupleLiteral(elements)
            | DefaultExpressionKindV1::ArrayLiteral(elements) => {
                self.push_expressions(pending, depth, elements)
            }
            DefaultExpressionKindV1::StructInit {
                constructor,
                arguments,
            }
            | DefaultExpressionKindV1::ClassInit {
                constructor,
                arguments,
            } => {
                self.push_expressions(pending, depth, arguments)?;
                self.push_child(
                    pending,
                    depth,
                    BodyNode::ConstructorRef {
                        constructor,
                        definition_origin,
                    },
                )
            }
            DefaultExpressionKindV1::StructConstruct { owner_type, fields } => {
                self.push_expressions(pending, depth, fields)?;
                self.push_type(
                    pending,
                    owner_type,
                    DefaultBodyProviderTypeSiteV1::StructConstructOwner,
                    definition_origin,
                )
            }
            DefaultExpressionKindV1::VariantConstruct { variant, arguments } => {
                self.push_expressions(pending, depth, arguments)?;
                self.push_child(
                    pending,
                    depth,
                    BodyNode::EnumVariantRef {
                        variant,
                        definition_origin,
                    },
                )
            }
            DefaultExpressionKindV1::VariantTest { operand, variant } => {
                self.push_child(
                    pending,
                    depth,
                    BodyNode::EnumVariantRef {
                        variant,
                        definition_origin,
                    },
                )?;
                self.push_child(pending, depth, BodyNode::Expression(operand))
            }
            DefaultExpressionKindV1::VariantPayloadProject { operand, field } => {
                self.push_child(
                    pending,
                    depth,
                    BodyNode::EnumVariantFieldRef {
                        field,
                        definition_origin,
                    },
                )?;
                self.push_child(pending, depth, BodyNode::Expression(operand))
            }
            DefaultExpressionKindV1::Lambda(lambda) => self.push_child(
                pending,
                depth,
                BodyNode::Lambda {
                    lambda,
                    definition_origin,
                },
            ),
            DefaultExpressionKindV1::AnonymousFunction(function) => self.push_child(
                pending,
                depth,
                BodyNode::AnonymousFunction {
                    function,
                    definition_origin,
                },
            ),
            DefaultExpressionKindV1::CallableReference(reference) => self.push_child(
                pending,
                depth,
                BodyNode::CallableReference {
                    reference,
                    definition_origin,
                },
            ),
            DefaultExpressionKindV1::FunctionCoercion {
                source,
                source_function_type,
                target_function_type,
            } => {
                self.push_type(
                    pending,
                    target_function_type,
                    DefaultBodyProviderTypeSiteV1::FunctionCoercionTarget,
                    definition_origin,
                )?;
                self.push_type(
                    pending,
                    source_function_type,
                    DefaultBodyProviderTypeSiteV1::FunctionCoercionSource,
                    definition_origin,
                )?;
                self.push_child(pending, depth, BodyNode::Expression(source))
            }
            DefaultExpressionKindV1::PtrFromNonZeroULong(operand)
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
            | DefaultExpressionKindV1::Unwrap { operand, .. } => {
                self.push_child(pending, depth, BodyNode::Expression(operand))
            }
            DefaultExpressionKindV1::PtrLoad { pointer, offset } => {
                self.push_optional_expression(pending, depth, offset.as_ref())?;
                self.push_child(pending, depth, BodyNode::Expression(pointer))
            }
            DefaultExpressionKindV1::PtrStore {
                pointer,
                offset,
                value,
            } => {
                self.push_child(pending, depth, BodyNode::Expression(value))?;
                self.push_optional_expression(pending, depth, offset.as_ref())?;
                self.push_child(pending, depth, BodyNode::Expression(pointer))
            }
            DefaultExpressionKindV1::PtrOffset {
                pointer, offset, ..
            }
            | DefaultExpressionKindV1::Index {
                receiver: pointer,
                index: offset,
                ..
            } => {
                self.push_child(pending, depth, BodyNode::Expression(offset))?;
                self.push_child(pending, depth, BodyNode::Expression(pointer))
            }
            DefaultExpressionKindV1::SizeOf(operand_type) => self.push_type(
                pending,
                operand_type,
                DefaultBodyProviderTypeSiteV1::SizeOfOperand,
                definition_origin,
            ),
            DefaultExpressionKindV1::AlignOf(operand_type) => self.push_type(
                pending,
                operand_type,
                DefaultBodyProviderTypeSiteV1::AlignOfOperand,
                definition_origin,
            ),
            DefaultExpressionKindV1::ForeignCallbackRegister { closure, .. } => {
                self.push_child(pending, depth, BodyNode::Expression(closure))
            }
            DefaultExpressionKindV1::ForeignCallbackOperation { callback, .. } => {
                self.push_child(pending, depth, BodyNode::Expression(callback))
            }
            DefaultExpressionKindV1::FieldAccess { receiver, field } => {
                self.push_child(
                    pending,
                    depth,
                    BodyNode::FieldRef {
                        field,
                        definition_origin,
                    },
                )?;
                self.push_child(pending, depth, BodyNode::Expression(receiver))
            }
            DefaultExpressionKindV1::MethodCall {
                receiver,
                callee,
                arguments,
            }
            | DefaultExpressionKindV1::DirectSuperMethodCall {
                receiver,
                callee,
                arguments,
            } => {
                self.push_expressions(pending, depth, arguments)?;
                self.push_child(
                    pending,
                    depth,
                    BodyNode::MethodCallee {
                        callee,
                        definition_origin,
                    },
                )?;
                self.push_child(pending, depth, BodyNode::Expression(receiver))
            }
            DefaultExpressionKindV1::IsInstance {
                operand,
                checked_type,
            }
            | DefaultExpressionKindV1::Cast {
                operand,
                checked_type,
                ..
            } => {
                self.push_type(
                    pending,
                    checked_type,
                    DefaultBodyProviderTypeSiteV1::InstanceCheck,
                    definition_origin,
                )?;
                self.push_child(pending, depth, BodyNode::Expression(operand))
            }
            DefaultExpressionKindV1::ArrayAssembly(assembly) => self.push_child(
                pending,
                depth,
                BodyNode::ArrayAssembly {
                    assembly,
                    definition_origin,
                },
            ),
            DefaultExpressionKindV1::ArraySet {
                receiver,
                index,
                value,
                ..
            } => {
                self.push_child(pending, depth, BodyNode::Expression(value))?;
                self.push_child(pending, depth, BodyNode::Expression(index))?;
                self.push_child(pending, depth, BodyNode::Expression(receiver))
            }
            DefaultExpressionKindV1::Call { callee, arguments } => {
                self.push_expressions(pending, depth, arguments)?;
                self.push_child(
                    pending,
                    depth,
                    BodyNode::CallableRef {
                        callable: callee,
                        definition_origin,
                    },
                )
            }
            DefaultExpressionKindV1::LocalFunctionCall {
                callee,
                captures,
                arguments,
                ..
            } => {
                self.push_expressions(pending, depth, arguments)?;
                self.push_expressions(pending, depth, captures)?;
                self.push_child(
                    pending,
                    depth,
                    BodyNode::CallableRef {
                        callable: callee,
                        definition_origin,
                    },
                )
            }
            DefaultExpressionKindV1::CallableCall {
                callee,
                function_type,
                arguments,
            } => {
                self.push_expressions(pending, depth, arguments)?;
                self.push_type(
                    pending,
                    function_type,
                    DefaultBodyProviderTypeSiteV1::CallableCallFunction,
                    definition_origin,
                )?;
                self.push_child(pending, depth, BodyNode::Expression(callee))
            }
            DefaultExpressionKindV1::PrimitiveBinary { lhs, rhs, .. }
            | DefaultExpressionKindV1::Binary { lhs, rhs, .. } => {
                self.push_child(pending, depth, BodyNode::Expression(rhs))?;
                self.push_child(pending, depth, BodyNode::Expression(lhs))
            }
            DefaultExpressionKindV1::IntegerOperation { arguments, .. } => {
                self.push_child(pending, depth, BodyNode::IntegerArguments(arguments))
            }
            DefaultExpressionKindV1::IntegerConversion { operand, .. } => {
                self.push_child(pending, depth, BodyNode::Expression(operand))
            }
        }
    }

    fn push_optional_expression<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        depth: u64,
        expression: Option<&'body DefaultExpressionV1>,
    ) -> Result<(), M::Error> {
        match expression {
            Some(expression) => self.push_child(pending, depth, BodyNode::Expression(expression)),
            None => Ok(()),
        }
    }

    pub(super) fn push_expressions<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        depth: u64,
        expressions: &'body [DefaultExpressionV1],
    ) -> Result<(), M::Error> {
        for expression in expressions.iter().rev() {
            self.push_child(pending, depth, BodyNode::Expression(expression))?;
        }
        Ok(())
    }

    pub(super) fn process_array_assembly<'body>(
        &mut self,
        assembly: &'body DefaultArrayAssemblyV1,
        definition_origin: &'body crate::ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        self.push_type(
            pending,
            assembly.result_type(),
            DefaultBodyProviderTypeSiteV1::ArrayAssemblyResult,
            definition_origin,
        )?;
        for part in assembly.parts().iter().rev() {
            let expression = match part {
                DefaultArrayAssemblyPartV1::Element(expression)
                | DefaultArrayAssemblyPartV1::CopyArray(expression) => expression,
            };
            self.push_child(pending, depth, BodyNode::Expression(expression))?;
        }
        self.push_type(
            pending,
            assembly.element_type(),
            DefaultBodyProviderTypeSiteV1::ArrayAssemblyElement,
            definition_origin,
        )
    }

    pub(super) fn process_integer_arguments<'body>(
        &mut self,
        arguments: &'body DefaultIntegerArgumentsV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        match arguments {
            DefaultIntegerArgumentsV1::Unary(operand) => {
                self.push_child(pending, depth, BodyNode::Expression(operand))
            }
            DefaultIntegerArgumentsV1::Binary { lhs, rhs } => {
                self.push_child(pending, depth, BodyNode::Expression(rhs))?;
                self.push_child(pending, depth, BodyNode::Expression(lhs))
            }
        }
    }
}
