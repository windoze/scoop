use crate::{
    DefaultArrayAssemblyPartV1, DefaultArrayAssemblyV1, DefaultExpressionKindV1,
    DefaultExpressionV1, DefaultIntegerArgumentsV1, DefaultIntegerOperationV1, DefaultPlaceV1,
};

use super::super::{
    CallableTargetView, ConstructorTargetView, ExportDefaultReferenceClosureValidationError,
    ExportDefaultReferenceOccurrenceSiteV1,
};
use super::{BodyNode, Validator, WorkItem};
use crate::DefaultBodyProviderTypeSiteV1;

impl Validator<'_> {
    pub(super) fn process_expression<'body>(
        &mut self,
        expression: &'body DefaultExpressionV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.process_expression_kind(
            expression.kind(),
            expression.definition_origin(),
            depth,
            pending,
        )?;
        self.push_type(
            pending,
            expression.result_type(),
            expression.definition_origin(),
            DefaultBodyProviderTypeSiteV1::ExpressionResult,
        )
    }

    fn process_expression_kind<'body>(
        &mut self,
        kind: &'body DefaultExpressionKindV1,
        origin: &'body crate::ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        match kind {
            DefaultExpressionKindV1::StringLiteral { .. }
            | DefaultExpressionKindV1::IntegerLiteral(_)
            | DefaultExpressionKindV1::BooleanLiteral(_)
            | DefaultExpressionKindV1::UnitLiteral
            | DefaultExpressionKindV1::Local(_)
            | DefaultExpressionKindV1::NoneLiteral => Ok(()),
            DefaultExpressionKindV1::GlobalRead(property) => self.push_global(
                pending,
                *property,
                origin,
                ExportDefaultReferenceOccurrenceSiteV1::Expression,
            ),
            DefaultExpressionKindV1::SingletonValue(value) => self.push_singleton(
                pending,
                *value,
                origin,
                ExportDefaultReferenceOccurrenceSiteV1::Expression,
            ),
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
                    BodyNode::ConstructorUse {
                        target: ConstructorTargetView::Constructor(constructor),
                        origin,
                        site: ExportDefaultReferenceOccurrenceSiteV1::Expression,
                    },
                )
            }
            DefaultExpressionKindV1::StructConstruct { owner_type, fields } => {
                self.push_expressions(pending, depth, fields)?;
                self.push_type(
                    pending,
                    owner_type,
                    origin,
                    DefaultBodyProviderTypeSiteV1::StructConstructOwner,
                )
            }
            DefaultExpressionKindV1::VariantConstruct { variant, arguments } => {
                self.push_expressions(pending, depth, arguments)?;
                self.push_child(
                    pending,
                    depth,
                    BodyNode::ConstructorUse {
                        target: ConstructorTargetView::Variant(variant),
                        origin,
                        site: ExportDefaultReferenceOccurrenceSiteV1::Expression,
                    },
                )
            }
            DefaultExpressionKindV1::VariantTest { operand, variant } => {
                self.push_child(
                    pending,
                    depth,
                    BodyNode::ConstructorUse {
                        target: ConstructorTargetView::Variant(variant),
                        origin,
                        site: ExportDefaultReferenceOccurrenceSiteV1::Expression,
                    },
                )?;
                self.push_child(pending, depth, BodyNode::Expression(operand))
            }
            DefaultExpressionKindV1::VariantPayloadProject { operand, field } => {
                self.push_child(
                    pending,
                    depth,
                    BodyNode::VariantFieldShape {
                        field,
                        origin,
                        site: DefaultBodyProviderTypeSiteV1::EnumVariantFieldOwner,
                    },
                )?;
                self.push_child(pending, depth, BodyNode::Expression(operand))
            }
            DefaultExpressionKindV1::Lambda(lambda) => {
                self.push_child(pending, depth, BodyNode::Lambda { lambda, origin })
            }
            DefaultExpressionKindV1::AnonymousFunction(function) => {
                self.push_child(pending, depth, BodyNode::Anonymous { function, origin })
            }
            DefaultExpressionKindV1::CallableReference(reference) => self.push_child(
                pending,
                depth,
                BodyNode::CallableReference { reference, origin },
            ),
            DefaultExpressionKindV1::FunctionCoercion {
                source,
                source_function_type,
                target_function_type,
            } => {
                self.push_type(
                    pending,
                    target_function_type,
                    origin,
                    DefaultBodyProviderTypeSiteV1::FunctionCoercionTarget,
                )?;
                self.push_type(
                    pending,
                    source_function_type,
                    origin,
                    DefaultBodyProviderTypeSiteV1::FunctionCoercionSource,
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
            | DefaultExpressionKindV1::Unwrap { operand, .. }
            | DefaultExpressionKindV1::Cast { operand, .. } => {
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
            DefaultExpressionKindV1::AddressOf(place) => match place {
                DefaultPlaceV1::Local { .. } => Ok(()),
                DefaultPlaceV1::Global { property } => self.push_global(
                    pending,
                    *property,
                    origin,
                    ExportDefaultReferenceOccurrenceSiteV1::Expression,
                ),
            },
            DefaultExpressionKindV1::SizeOf(operand_type) => self.push_type(
                pending,
                operand_type,
                origin,
                DefaultBodyProviderTypeSiteV1::SizeOfOperand,
            ),
            DefaultExpressionKindV1::AlignOf(operand_type) => self.push_type(
                pending,
                operand_type,
                origin,
                DefaultBodyProviderTypeSiteV1::AlignOfOperand,
            ),
            DefaultExpressionKindV1::FunctionAddress(declaration) => self.push_callable(
                pending,
                CallableTargetView::FunctionAddress(*declaration),
                origin,
                ExportDefaultReferenceOccurrenceSiteV1::Expression,
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
                    BodyNode::FieldUse {
                        target: super::super::FieldTargetView::Field(field),
                        origin,
                        site: ExportDefaultReferenceOccurrenceSiteV1::Expression,
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
                self.push_child(pending, depth, BodyNode::MethodCallee { callee, origin })?;
                self.push_child(pending, depth, BodyNode::Expression(receiver))
            }
            DefaultExpressionKindV1::IsInstance {
                operand,
                checked_type,
            } => {
                self.push_type(
                    pending,
                    checked_type,
                    origin,
                    DefaultBodyProviderTypeSiteV1::InstanceCheck,
                )?;
                self.push_child(pending, depth, BodyNode::Expression(operand))
            }
            DefaultExpressionKindV1::ArrayAssembly(assembly) => {
                self.push_child(pending, depth, BodyNode::ArrayAssembly { assembly, origin })
            }
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
                    BodyNode::CallableUse {
                        callable: callee,
                        origin,
                    },
                )
            }
            DefaultExpressionKindV1::LocalFunctionCall {
                declaration,
                callee,
                captures,
                arguments,
            } => {
                self.push_expressions(pending, depth, arguments)?;
                self.push_expressions(pending, depth, captures)?;
                self.push_child(
                    pending,
                    depth,
                    BodyNode::CallableUse {
                        callable: callee,
                        origin,
                    },
                )?;
                self.push_callable(
                    pending,
                    CallableTargetView::LocalFunction(*declaration),
                    origin,
                    ExportDefaultReferenceOccurrenceSiteV1::Expression,
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
                    origin,
                    DefaultBodyProviderTypeSiteV1::CallableCallFunction,
                )?;
                self.push_child(pending, depth, BodyNode::Expression(callee))
            }
            DefaultExpressionKindV1::PrimitiveBinary { lhs, rhs, .. }
            | DefaultExpressionKindV1::Binary { lhs, rhs, .. } => {
                self.push_child(pending, depth, BodyNode::Expression(rhs))?;
                self.push_child(pending, depth, BodyNode::Expression(lhs))
            }
            DefaultExpressionKindV1::IntegerOperation {
                operation,
                arguments,
            } => {
                self.push_child(pending, depth, BodyNode::IntegerArguments(arguments))?;
                self.push_child(
                    pending,
                    depth,
                    BodyNode::IntegerOperation { operation, origin },
                )
            }
            DefaultExpressionKindV1::IntegerConversion {
                target, operand, ..
            } => {
                self.push_child(pending, depth, BodyNode::Expression(operand))?;
                self.push_child(
                    pending,
                    depth,
                    BodyNode::CallableUse {
                        callable: target,
                        origin,
                    },
                )
            }
        }
    }

    fn push_optional_expression<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        depth: u64,
        expression: Option<&'body DefaultExpressionV1>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
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
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        for expression in expressions.iter().rev() {
            self.push_child(pending, depth, BodyNode::Expression(expression))?;
        }
        Ok(())
    }

    pub(super) fn process_array_assembly<'body>(
        &mut self,
        assembly: &'body DefaultArrayAssemblyV1,
        origin: &'body crate::ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.push_type(
            pending,
            assembly.result_type(),
            origin,
            DefaultBodyProviderTypeSiteV1::ArrayAssemblyResult,
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
            origin,
            DefaultBodyProviderTypeSiteV1::ArrayAssemblyElement,
        )
    }

    pub(super) fn process_integer_operation<'body>(
        &mut self,
        operation: &'body DefaultIntegerOperationV1,
        origin: &'body crate::ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        let callable = match operation {
            DefaultIntegerOperationV1::NoGc { target, .. }
            | DefaultIntegerOperationV1::Managed { target, .. } => target,
        };
        self.push_child(pending, depth, BodyNode::CallableUse { callable, origin })
    }

    pub(super) fn process_integer_arguments<'body>(
        &mut self,
        arguments: &'body DefaultIntegerArgumentsV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
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
