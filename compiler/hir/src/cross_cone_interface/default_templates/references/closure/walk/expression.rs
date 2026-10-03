use crate::{
    DefaultArrayAssemblyPartV1, DefaultArrayAssemblyV1, DefaultExpressionKindV1,
    DefaultExpressionV1, DefaultIntegerArgumentsV1, DefaultPlaceV1,
};

use super::super::{
    CallableTargetView, ConstructorTargetView, ExportDefaultReferenceOccurrenceSiteV1,
};
use super::{BodyNode, DefaultBodyReferenceVisitorV1, ReferenceWalker, ScheduledWork};
use crate::DefaultBodyProviderTypeSiteV1;

impl<'body, V: DefaultBodyReferenceVisitorV1<'body>> ReferenceWalker<'_, 'body, V> {
    pub(super) fn process_expression(
        &mut self,
        expression: &'body DefaultExpressionV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        self.process_expression_kind(expression.kind(), expression.definition_origin(), pending)?;
        self.push_type(
            pending,
            expression.result_type(),
            expression.definition_origin(),
            DefaultBodyProviderTypeSiteV1::ExpressionResult,
        )
    }

    fn process_expression_kind(
        &mut self,
        kind: &'body DefaultExpressionKindV1,
        origin: &'body crate::ExportDefinitionSourceV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        match kind {
            DefaultExpressionKindV1::GenericDelegateStorageRead(reference) => self
                .push_generic_delegate(
                    pending,
                    reference,
                    origin,
                    ExportDefaultReferenceOccurrenceSiteV1::Expression,
                ),
            DefaultExpressionKindV1::StringLiteral { .. }
            | DefaultExpressionKindV1::IntegerLiteral(_)
            | DefaultExpressionKindV1::BooleanLiteral(_)
            | DefaultExpressionKindV1::UnitLiteral
            | DefaultExpressionKindV1::Local(_)
            | DefaultExpressionKindV1::Capture(_)
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
                self.push_expressions(pending, elements)
            }
            DefaultExpressionKindV1::StructInit {
                constructor,
                arguments,
            }
            | DefaultExpressionKindV1::ClassInit {
                constructor,
                arguments,
            } => {
                self.push_expressions(pending, arguments)?;
                self.push_child(
                    pending,
                    BodyNode::ConstructorUse {
                        target: ConstructorTargetView::Constructor(constructor),
                        origin,
                        site: ExportDefaultReferenceOccurrenceSiteV1::Expression,
                    },
                )
            }
            DefaultExpressionKindV1::StructConstruct { owner_type, fields } => {
                self.push_expressions(pending, fields)?;
                self.push_type(
                    pending,
                    owner_type,
                    origin,
                    DefaultBodyProviderTypeSiteV1::StructConstructOwner,
                )
            }
            DefaultExpressionKindV1::VariantConstruct { variant, arguments } => {
                self.push_expressions(pending, arguments)?;
                self.push_child(
                    pending,
                    BodyNode::ConstructorUse {
                        target: ConstructorTargetView::Variant(variant),
                        origin,
                        site: ExportDefaultReferenceOccurrenceSiteV1::Expression,
                    },
                )
            }
            DefaultExpressionKindV1::VariantTest { operand, variant } => {
                self.push_type(
                    pending,
                    variant.owner_type(),
                    origin,
                    DefaultBodyProviderTypeSiteV1::EnumVariantOwner,
                )?;
                self.push_child(pending, BodyNode::Expression(operand))
            }
            DefaultExpressionKindV1::VariantPayloadProject { operand, field } => {
                self.push_child(
                    pending,
                    BodyNode::VariantFieldShape {
                        field,
                        origin,
                        site: DefaultBodyProviderTypeSiteV1::EnumVariantFieldOwner,
                    },
                )?;
                self.push_child(pending, BodyNode::Expression(operand))
            }
            DefaultExpressionKindV1::Lambda(lambda) => {
                self.push_child(pending, BodyNode::Lambda { lambda, origin })
            }
            DefaultExpressionKindV1::AnonymousFunction(function) => {
                self.push_child(pending, BodyNode::Anonymous { function, origin })
            }
            DefaultExpressionKindV1::CallableReference(reference) => {
                self.push_child(pending, BodyNode::CallableReference { reference, origin })
            }
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
                self.push_child(pending, BodyNode::Expression(source))
            }
            DefaultExpressionKindV1::PtrFromNonZeroULong(operand)
            | DefaultExpressionKindV1::PtrToULong(operand)
            | DefaultExpressionKindV1::PtrCast(operand)
            | DefaultExpressionKindV1::Box(operand)
            | DefaultExpressionKindV1::Unbox(operand)
            | DefaultExpressionKindV1::ReferenceUpcast(operand)
            | DefaultExpressionKindV1::ArrayLen(operand)
            | DefaultExpressionKindV1::ArrayClone(operand)
            | DefaultExpressionKindV1::PrimitiveUnary { operand, .. }
            | DefaultExpressionKindV1::Unary { operand, .. }
            | DefaultExpressionKindV1::SomeWrap(operand)
            | DefaultExpressionKindV1::IsSome(operand)
            | DefaultExpressionKindV1::Unwrap { operand, .. } => {
                self.push_child(pending, BodyNode::Expression(operand))
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
                self.push_child(pending, BodyNode::Expression(closure))
            }
            DefaultExpressionKindV1::ForeignCallbackOperation { callback, .. } => {
                self.push_child(pending, BodyNode::Expression(callback))
            }
            DefaultExpressionKindV1::FieldAccess { receiver, field } => {
                self.push_child(
                    pending,
                    BodyNode::FieldUse {
                        target: super::super::FieldTargetView::Field(field),
                        origin,
                        site: ExportDefaultReferenceOccurrenceSiteV1::Expression,
                    },
                )?;
                self.push_child(pending, BodyNode::Expression(receiver))
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
                self.push_expressions(pending, arguments)?;
                self.push_child(pending, BodyNode::MethodCallee { callee, origin })?;
                self.push_child(pending, BodyNode::Expression(receiver))
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
                    origin,
                    DefaultBodyProviderTypeSiteV1::InstanceCheck,
                )?;
                self.push_child(pending, BodyNode::Expression(operand))
            }
            DefaultExpressionKindV1::ArrayAssembly(assembly) => {
                self.push_child(pending, BodyNode::ArrayAssembly { assembly, origin })
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
            DefaultExpressionKindV1::Call {
                callee,
                arguments,
                receiver,
            } => {
                if let crate::SourceCallReceiver::Receiver { static_type } = receiver {
                    self.push_type(
                        pending,
                        static_type,
                        origin,
                        DefaultBodyProviderTypeSiteV1::CallReceiver,
                    )?;
                }
                self.push_expressions(pending, arguments)?;
                self.push_child(
                    pending,
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
                self.push_expressions(pending, arguments)?;
                self.push_expressions(pending, captures)?;
                self.push_child(
                    pending,
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
                self.push_expressions(pending, arguments)?;
                self.push_type(
                    pending,
                    function_type,
                    origin,
                    DefaultBodyProviderTypeSiteV1::CallableCallFunction,
                )?;
                self.push_child(pending, BodyNode::Expression(callee))
            }
            DefaultExpressionKindV1::PrimitiveBinary { lhs, rhs, .. }
            | DefaultExpressionKindV1::Binary { lhs, rhs, .. } => {
                self.push_child(pending, BodyNode::Expression(rhs))?;
                self.push_child(pending, BodyNode::Expression(lhs))
            }
            DefaultExpressionKindV1::IntegerOperation { arguments, .. } => {
                self.push_child(pending, BodyNode::IntegerArguments(arguments))
            }
            DefaultExpressionKindV1::IntegerConversion { operand, .. } => {
                self.push_child(pending, BodyNode::Expression(operand))
            }
        }
    }

    fn push_optional_expression(
        &mut self,
        pending: &mut Vec<ScheduledWork<'body>>,
        expression: Option<&'body DefaultExpressionV1>,
    ) -> Result<(), V::Error> {
        match expression {
            Some(expression) => self.push_child(pending, BodyNode::Expression(expression)),
            None => Ok(()),
        }
    }

    pub(super) fn push_expressions(
        &mut self,
        pending: &mut Vec<ScheduledWork<'body>>,
        expressions: &'body [DefaultExpressionV1],
    ) -> Result<(), V::Error> {
        for expression in expressions.iter().rev() {
            self.push_child(pending, BodyNode::Expression(expression))?;
        }
        Ok(())
    }

    pub(super) fn process_array_assembly(
        &mut self,
        assembly: &'body DefaultArrayAssemblyV1,
        origin: &'body crate::ExportDefinitionSourceV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
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
            self.push_child(pending, BodyNode::Expression(expression))?;
        }
        self.push_type(
            pending,
            assembly.element_type(),
            origin,
            DefaultBodyProviderTypeSiteV1::ArrayAssemblyElement,
        )
    }

    pub(super) fn process_integer_arguments(
        &mut self,
        arguments: &'body DefaultIntegerArgumentsV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
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
