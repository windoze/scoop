use scoop_identity::{CallableTemplateOrigin, Effect, SignatureTypeKey};

use crate::{
    DefaultArrayAccessKindV1, DefaultArrayAssemblyPartV1, DefaultBinaryOperatorV1,
    DefaultCallableDeclarationV1, DefaultCallableOperationShapeV1,
    DefaultCallableReferenceTargetV1, DefaultCoreApplicationV1, DefaultExpressionOperationV1,
    DefaultFieldOperationKindV1, DefaultForeignCallbackOperationV1, DefaultIntegerArgumentsV1,
    DefaultIntegerKindV1, DefaultIntegerOperationV1, DefaultNoGcIntegerOperationV1,
    DefaultOperationCoreTypeV1, DefaultOperationEntityShapeKindV1, DefaultOperationEntityShapeV1,
    DefaultOperationEntityV1, DefaultOperationExpectedTypeShapeV1, DefaultOperationIntrinsicV1,
    DefaultOperationTypeRelationV1, DefaultOperationTypingProblemV1,
    DefaultOperationTypingSemanticAuthority, DefaultOperationValueRoleV1, DefaultPlaceV1,
    DefaultPrimitiveBinaryKindV1, DefaultPrimitiveUnaryKindV1, DefaultUnaryOperatorV1,
    ExportDefaultOperationTypingValidationError, ExportDefaultTemplateV1,
};

use super::{
    DefaultBodyOperationAuthority, DefaultBodyValidationInputV1, Validator,
    authority::PublicAuthority, expression_operation,
};

impl crate::DefaultExpressionV1 {
    /// Validates this complete expression subtree against canonical provider
    /// and trusted-core operation facts. Statement, pattern, and iteration
    /// relationships are validated by the enclosing body pass.
    pub fn validate_operation_typing_semantics<A, E>(
        &self,
        template: &ExportDefaultTemplateV1,
        authority: &mut A,
        meter: &mut scoop_wire::BudgetMeter,
        path: &scoop_wire::WirePath,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>>
    where
        A: DefaultOperationTypingSemanticAuthority<E>,
    {
        let mut adapter = PublicAuthority {
            template,
            authority,
        };
        Validator {
            template: DefaultBodyValidationInputV1::from(template),
            authority: &mut adapter,
            meter,
            path,
            error: std::marker::PhantomData,
        }
        .run_expression_at(self, 1)
    }
}

impl<A, E> Validator<'_, A, E>
where
    A: DefaultBodyOperationAuthority<E>,
{
    pub(super) fn run_expression_at(
        &mut self,
        expression: &crate::DefaultExpressionV1,
        depth: u64,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        let mut pending = Vec::new();
        self.meter
            .try_reserve_collection_slots(&mut pending, 1, self.path)
            .map_err(ExportDefaultOperationTypingValidationError::Resource)?;
        pending.push(ExpressionWork { expression, depth });
        while let Some(work) = pending.pop() {
            self.enter_node(work.depth)?;
            self.process_expression(work.expression, work.depth, &mut pending)?;
        }
        Ok(())
    }

    fn process_expression<'body>(
        &mut self,
        expression: &'body crate::DefaultExpressionV1,
        depth: u64,
        pending: &mut Vec<ExpressionWork<'body>>,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        let operation = expression_operation(expression.kind());
        match expression.kind() {
            crate::DefaultExpressionKindV1::StringLiteral { .. } => {
                let principal = self.core_type(
                    DefaultOperationCoreTypeV1::String,
                    Self::site(operation, DefaultOperationValueRoleV1::Result),
                )?;
                self.expect_result(expression, operation, &principal, true)
            }
            crate::DefaultExpressionKindV1::IntegerLiteral(value) => {
                let principal = self.integer_type(
                    DefaultIntegerKindV1::from(value.kind()),
                    operation,
                    DefaultOperationValueRoleV1::Result,
                )?;
                self.expect_result(expression, operation, &principal, false)
            }
            crate::DefaultExpressionKindV1::BooleanLiteral(_) => {
                let principal = self.core_type(
                    DefaultOperationCoreTypeV1::Boolean,
                    Self::site(operation, DefaultOperationValueRoleV1::Result),
                )?;
                self.expect_result(expression, operation, &principal, false)
            }
            crate::DefaultExpressionKindV1::UnitLiteral => {
                let principal = self.core_type(
                    DefaultOperationCoreTypeV1::Unit,
                    Self::site(operation, DefaultOperationValueRoleV1::Result),
                )?;
                self.expect_result(expression, operation, &principal, false)
            }
            crate::DefaultExpressionKindV1::TupleLiteral(elements) => {
                if elements.is_empty() {
                    return self.problem(
                        operation,
                        DefaultOperationValueRoleV1::Result,
                        DefaultOperationTypingProblemV1::EmptyTuple,
                    );
                }
                let SignatureTypeKey::Tuple(result_elements) = expression.result_type() else {
                    return Err(ExportDefaultOperationTypingValidationError::TypeShape {
                        site: Self::site(operation, DefaultOperationValueRoleV1::Result),
                        expected: DefaultOperationExpectedTypeShapeV1::Tuple,
                        actual: Box::new(expression.result_type().clone()),
                    });
                };
                self.expect_arity(
                    result_elements.as_slice().len(),
                    elements.len(),
                    Self::site(operation, DefaultOperationValueRoleV1::Result),
                )?;
                for (index, (actual, expected)) in
                    result_elements.as_slice().iter().zip(elements).enumerate()
                {
                    self.expect_type(
                        actual,
                        expected.result_type(),
                        Self::site(operation, DefaultOperationValueRoleV1::Field { index }),
                    )?;
                }
                self.push_expressions(pending, elements, depth)
            }
            crate::DefaultExpressionKindV1::StructInit {
                constructor,
                arguments,
            } => {
                let (owner, parameters) = self.constructor_shape(constructor, operation)?;
                self.expect_type(
                    constructor.owner_type(),
                    &owner,
                    Self::site(operation, DefaultOperationValueRoleV1::Target),
                )?;
                self.expect_arguments(operation, arguments, &parameters)?;
                self.expect_result(expression, operation, &owner, true)?;
                self.push_expressions(pending, arguments, depth)
            }
            crate::DefaultExpressionKindV1::ClassInit {
                constructor,
                arguments,
            } => {
                let (owner, parameters) = self.constructor_shape(constructor, operation)?;
                self.expect_type(
                    constructor.owner_type(),
                    &owner,
                    Self::site(operation, DefaultOperationValueRoleV1::Target),
                )?;
                self.expect_arguments(operation, arguments, &parameters)?;
                self.expect_result(expression, operation, &owner, true)?;
                self.push_expressions(pending, arguments, depth)
            }
            crate::DefaultExpressionKindV1::StructConstruct { owner_type, fields } => {
                let shape = self.aggregate_shape(
                    DefaultOperationEntityV1::Struct(owner_type),
                    operation,
                    DefaultOperationValueRoleV1::Target,
                )?;
                self.expect_type(
                    owner_type,
                    shape.owner_type(),
                    Self::site(operation, DefaultOperationValueRoleV1::Target),
                )?;
                self.expect_arguments_with_role(operation, fields, shape.fields(), |index| {
                    DefaultOperationValueRoleV1::Field { index }
                })?;
                self.expect_result(expression, operation, owner_type, true)?;
                self.push_expressions(pending, fields, depth)
            }
            crate::DefaultExpressionKindV1::VariantConstruct { variant, arguments } => {
                let shape = self.aggregate_shape(
                    DefaultOperationEntityV1::Variant(variant),
                    operation,
                    DefaultOperationValueRoleV1::Target,
                )?;
                self.expect_type(
                    variant.owner_type(),
                    shape.owner_type(),
                    Self::site(operation, DefaultOperationValueRoleV1::Target),
                )?;
                self.expect_arguments(operation, arguments, shape.fields())?;
                self.expect_result(expression, operation, variant.owner_type(), true)?;
                self.push_expressions(pending, arguments, depth)
            }
            crate::DefaultExpressionKindV1::VariantTest { operand, variant } => {
                let shape = self.aggregate_shape(
                    DefaultOperationEntityV1::Variant(variant),
                    operation,
                    DefaultOperationValueRoleV1::Target,
                )?;
                self.expect_type(
                    variant.owner_type(),
                    shape.owner_type(),
                    Self::site(operation, DefaultOperationValueRoleV1::Target),
                )?;
                self.expect_type(
                    operand.result_type(),
                    variant.owner_type(),
                    Self::site(operation, DefaultOperationValueRoleV1::Operand),
                )?;
                let boolean = self.core_type(
                    DefaultOperationCoreTypeV1::Boolean,
                    Self::site(operation, DefaultOperationValueRoleV1::Result),
                )?;
                self.expect_result(expression, operation, &boolean, false)?;
                self.push_expression(pending, operand, depth)
            }
            crate::DefaultExpressionKindV1::VariantPayloadProject { operand, field } => {
                let shape = self.variant_field_shape(field, operation)?;
                self.expect_type(
                    field.owner_type(),
                    shape.owner_type(),
                    Self::site(operation, DefaultOperationValueRoleV1::Target),
                )?;
                self.expect_type(
                    operand.result_type(),
                    field.owner_type(),
                    Self::site(operation, DefaultOperationValueRoleV1::Operand),
                )?;
                self.expect_result(expression, operation, shape.value_type(), true)?;
                self.push_expression(pending, operand, depth)
            }
            crate::DefaultExpressionKindV1::Local(local) => {
                self.charge_work()?;
                let principal = self
                    .template
                    .locals()
                    .get(local)
                    .map(|record| record.value_type().clone())
                    .ok_or_else(|| ExportDefaultOperationTypingValidationError::Problem {
                        site: Self::site(operation, DefaultOperationValueRoleV1::Operand),
                        problem: DefaultOperationTypingProblemV1::MissingLocal,
                    })?;
                self.expect_result(expression, operation, &principal, true)
            }
            crate::DefaultExpressionKindV1::GlobalRead(property) => {
                let shape = self.value_shape(
                    DefaultOperationEntityV1::Global(*property),
                    operation,
                    DefaultOperationValueRoleV1::Target,
                )?;
                self.expect_result(expression, operation, shape.value_type(), true)
            }
            crate::DefaultExpressionKindV1::SingletonValue(singleton) => {
                let principal = self.type_shape(
                    DefaultOperationEntityV1::Singleton(*singleton),
                    operation,
                    DefaultOperationValueRoleV1::Target,
                )?;
                self.expect_result(expression, operation, &principal, true)
            }
            crate::DefaultExpressionKindV1::Lambda(lambda) => {
                self.validate_nested_callable_type(expression, operation, lambda.function_type())
            }
            crate::DefaultExpressionKindV1::AnonymousFunction(function) => {
                self.validate_nested_callable_type(expression, operation, function.function_type())
            }
            crate::DefaultExpressionKindV1::CallableReference(reference) => {
                match reference.target() {
                    DefaultCallableReferenceTargetV1::Named(callee) => {
                        let shape = self.callable_shape(
                            DefaultOperationEntityV1::Callable(callee),
                            operation,
                            DefaultOperationValueRoleV1::Callable,
                        )?;
                        self.ensure_no_captures(operation, &shape)?;
                        self.expect_callable_reference_type(
                            operation,
                            &shape,
                            true,
                            reference.function_type(),
                        )?;
                    }
                    DefaultCallableReferenceTargetV1::Local {
                        declaration,
                        callee,
                    } => {
                        self.expect_local_declaration(
                            operation,
                            *declaration,
                            callee.declaration(),
                        )?;
                        let shape = self.callable_shape(
                            DefaultOperationEntityV1::Callable(callee),
                            operation,
                            DefaultOperationValueRoleV1::Callable,
                        )?;
                        if shape.receiver().is_some() {
                            return self.problem(
                                operation,
                                DefaultOperationValueRoleV1::Receiver,
                                DefaultOperationTypingProblemV1::UnexpectedCallableReceiver,
                            );
                        }
                        self.expect_callable_reference_type(
                            operation,
                            &shape,
                            false,
                            reference.function_type(),
                        )?;
                    }
                    DefaultCallableReferenceTargetV1::BoundMember { receiver, callee } => {
                        let shape = self.callable_shape(
                            DefaultOperationEntityV1::MethodCallee(callee),
                            operation,
                            DefaultOperationValueRoleV1::Callable,
                        )?;
                        let expected = shape.receiver().ok_or_else(|| {
                            ExportDefaultOperationTypingValidationError::Problem {
                                site: Self::site(operation, DefaultOperationValueRoleV1::Receiver),
                                problem: DefaultOperationTypingProblemV1::MissingCallableReceiver,
                            }
                        })?;
                        self.expect_relation(
                            DefaultOperationTypeRelationV1::MemberReceiver,
                            receiver.result_type(),
                            expected,
                            Self::site(operation, DefaultOperationValueRoleV1::Receiver),
                        )?;
                        self.ensure_no_captures(operation, &shape)?;
                        self.expect_callable_reference_type(
                            operation,
                            &shape,
                            false,
                            reference.function_type(),
                        )?;
                        self.push_expression(pending, receiver, depth)?;
                    }
                    DefaultCallableReferenceTargetV1::BoundExtension { receiver, callee } => {
                        let shape = self.callable_shape(
                            DefaultOperationEntityV1::Callable(callee),
                            operation,
                            DefaultOperationValueRoleV1::Callable,
                        )?;
                        let expected = shape.receiver().ok_or_else(|| {
                            ExportDefaultOperationTypingValidationError::Problem {
                                site: Self::site(operation, DefaultOperationValueRoleV1::Receiver),
                                problem: DefaultOperationTypingProblemV1::MissingCallableReceiver,
                            }
                        })?;
                        self.expect_type(
                            receiver.result_type(),
                            expected,
                            Self::site(operation, DefaultOperationValueRoleV1::Receiver),
                        )?;
                        self.ensure_no_captures(operation, &shape)?;
                        self.expect_callable_reference_type(
                            operation,
                            &shape,
                            false,
                            reference.function_type(),
                        )?;
                        self.push_expression(pending, receiver, depth)?;
                    }
                }
                self.expect_result(expression, operation, reference.function_type(), true)
            }
            crate::DefaultExpressionKindV1::FunctionCoercion {
                source,
                source_function_type,
                target_function_type,
            } => {
                self.expect_function(
                    source_function_type,
                    Self::site(operation, DefaultOperationValueRoleV1::Source),
                )?;
                self.expect_function(
                    target_function_type,
                    Self::site(operation, DefaultOperationValueRoleV1::Target),
                )?;
                self.expect_type(
                    source.result_type(),
                    source_function_type,
                    Self::site(operation, DefaultOperationValueRoleV1::Source),
                )?;
                self.expect_relation(
                    DefaultOperationTypeRelationV1::FunctionCoercion,
                    source_function_type,
                    target_function_type,
                    Self::site(operation, DefaultOperationValueRoleV1::Target),
                )?;
                self.expect_result(expression, operation, target_function_type, false)?;
                self.push_expression(pending, source, depth)
            }
            crate::DefaultExpressionKindV1::PtrFromNonZeroULong(operand) => {
                let ulong = self.integer_type(
                    DefaultIntegerKindV1::Unsigned64,
                    operation,
                    DefaultOperationValueRoleV1::Operand,
                )?;
                self.expect_type(
                    operand.result_type(),
                    &ulong,
                    Self::site(operation, DefaultOperationValueRoleV1::Operand),
                )?;
                self.expect_raw_pointer(
                    expression.result_type(),
                    Self::site(operation, DefaultOperationValueRoleV1::Result),
                )?;
                self.push_expression(pending, operand, depth)
            }
            crate::DefaultExpressionKindV1::PtrToULong(operand) => {
                self.expect_raw_pointer(
                    operand.result_type(),
                    Self::site(operation, DefaultOperationValueRoleV1::Operand),
                )?;
                let ulong = self.integer_type(
                    DefaultIntegerKindV1::Unsigned64,
                    operation,
                    DefaultOperationValueRoleV1::Result,
                )?;
                self.expect_result(expression, operation, &ulong, false)?;
                self.push_expression(pending, operand, depth)
            }
            crate::DefaultExpressionKindV1::PtrCast(operand) => {
                self.expect_raw_pointer(
                    operand.result_type(),
                    Self::site(operation, DefaultOperationValueRoleV1::Operand),
                )?;
                self.expect_raw_pointer(
                    expression.result_type(),
                    Self::site(operation, DefaultOperationValueRoleV1::Result),
                )?;
                self.push_expression(pending, operand, depth)
            }
            crate::DefaultExpressionKindV1::PtrLoad { pointer, offset } => {
                let pointee = self
                    .expect_raw_pointer(
                        pointer.result_type(),
                        Self::site(operation, DefaultOperationValueRoleV1::Pointer),
                    )?
                    .clone();
                self.validate_optional_offset(operation, offset.as_ref())?;
                self.expect_result(expression, operation, &pointee, false)?;
                if let Some(offset) = offset.as_ref() {
                    self.push_expression(pending, offset, depth)?;
                }
                self.push_expression(pending, pointer, depth)
            }
            crate::DefaultExpressionKindV1::PtrStore {
                pointer,
                offset,
                value,
            } => {
                let pointee = self
                    .expect_raw_pointer(
                        pointer.result_type(),
                        Self::site(operation, DefaultOperationValueRoleV1::Pointer),
                    )?
                    .clone();
                self.validate_optional_offset(operation, offset.as_ref())?;
                self.expect_type(
                    value.result_type(),
                    &pointee,
                    Self::site(operation, DefaultOperationValueRoleV1::Value),
                )?;
                let unit = self.core_type(
                    DefaultOperationCoreTypeV1::Unit,
                    Self::site(operation, DefaultOperationValueRoleV1::Result),
                )?;
                self.expect_result(expression, operation, &unit, false)?;
                self.push_expression(pending, value, depth)?;
                if let Some(offset) = offset.as_ref() {
                    self.push_expression(pending, offset, depth)?;
                }
                self.push_expression(pending, pointer, depth)
            }
            crate::DefaultExpressionKindV1::PtrOffset {
                pointer, offset, ..
            } => {
                self.expect_raw_pointer(
                    pointer.result_type(),
                    Self::site(operation, DefaultOperationValueRoleV1::Pointer),
                )?;
                let long = self.integer_type(
                    DefaultIntegerKindV1::Signed64,
                    operation,
                    DefaultOperationValueRoleV1::Offset,
                )?;
                self.expect_type(
                    offset.result_type(),
                    &long,
                    Self::site(operation, DefaultOperationValueRoleV1::Offset),
                )?;
                self.expect_result(expression, operation, pointer.result_type(), false)?;
                self.push_expression(pending, offset, depth)?;
                self.push_expression(pending, pointer, depth)
            }
            crate::DefaultExpressionKindV1::AddressOf(place) => {
                let value_type = self.place_type(place, operation)?;
                self.expect_relation(
                    DefaultOperationTypeRelationV1::ConcreteGcFreeValue,
                    &value_type,
                    &value_type,
                    Self::site(operation, DefaultOperationValueRoleV1::Place),
                )?;
                let result = SignatureTypeKey::RawPointer(Box::new(value_type));
                self.expect_result(expression, operation, &result, false)
            }
            crate::DefaultExpressionKindV1::SizeOf(queried)
            | crate::DefaultExpressionKindV1::AlignOf(queried) => {
                self.expect_relation(
                    DefaultOperationTypeRelationV1::ConcreteGcFreeValue,
                    queried,
                    queried,
                    Self::site(operation, DefaultOperationValueRoleV1::QueriedType),
                )?;
                let ulong = self.integer_type(
                    DefaultIntegerKindV1::Unsigned64,
                    operation,
                    DefaultOperationValueRoleV1::Result,
                )?;
                self.expect_result(expression, operation, &ulong, false)
            }
            crate::DefaultExpressionKindV1::FunctionAddress(declaration) => {
                let result = self.type_shape(
                    DefaultOperationEntityV1::FunctionAddress(*declaration),
                    operation,
                    DefaultOperationValueRoleV1::Target,
                )?;
                self.expect_native_function_pointer(
                    &result,
                    Self::site(operation, DefaultOperationValueRoleV1::Target),
                )?;
                self.expect_result(expression, operation, &result, false)
            }
            crate::DefaultExpressionKindV1::ForeignCallbackRegister {
                registration,
                closure,
            } => {
                let shape = self.callback_shape(*registration, operation)?;
                self.expect_function(
                    shape.closure_type(),
                    Self::site(operation, DefaultOperationValueRoleV1::Target),
                )?;
                let callback = self.core_application(
                    shape.callback_type(),
                    DefaultOperationExpectedTypeShapeV1::ForeignCallback,
                    Self::site(operation, DefaultOperationValueRoleV1::Target),
                )?;
                self.expect_type(
                    callback.element(),
                    shape.closure_type(),
                    Self::site(operation, DefaultOperationValueRoleV1::Target),
                )?;
                self.expect_type(
                    closure.result_type(),
                    shape.closure_type(),
                    Self::site(operation, DefaultOperationValueRoleV1::Operand),
                )?;
                self.expect_result(expression, operation, shape.callback_type(), false)?;
                self.push_expression(pending, closure, depth)
            }
            crate::DefaultExpressionKindV1::ForeignCallbackOperation {
                operation: callback_operation,
                callback,
            } => {
                let application = self.core_application(
                    callback.result_type(),
                    DefaultOperationExpectedTypeShapeV1::ForeignCallback,
                    Self::site(operation, DefaultOperationValueRoleV1::Operand),
                )?;
                self.expect_function(
                    application.element(),
                    Self::site(operation, DefaultOperationValueRoleV1::Operand),
                )?;
                match callback_operation {
                    DefaultForeignCallbackOperationV1::Retain => {
                        self.expect_result(expression, operation, callback.result_type(), false)?
                    }
                    DefaultForeignCallbackOperationV1::Release => {
                        let unit = self.core_type(
                            DefaultOperationCoreTypeV1::Unit,
                            Self::site(operation, DefaultOperationValueRoleV1::Result),
                        )?;
                        self.expect_result(expression, operation, &unit, false)?;
                    }
                    DefaultForeignCallbackOperationV1::State => {
                        let state = self.core_type(
                            DefaultOperationCoreTypeV1::ForeignCallbackState,
                            Self::site(operation, DefaultOperationValueRoleV1::Result),
                        )?;
                        self.expect_result(expression, operation, &state, false)?;
                    }
                    DefaultForeignCallbackOperationV1::Failure => {
                        let option = self.core_application(
                            expression.result_type(),
                            DefaultOperationExpectedTypeShapeV1::Option,
                            Self::site(operation, DefaultOperationValueRoleV1::Result),
                        )?;
                        let throwable = self.core_type(
                            DefaultOperationCoreTypeV1::Throwable,
                            Self::site(operation, DefaultOperationValueRoleV1::Result),
                        )?;
                        self.expect_type(
                            option.element(),
                            &throwable,
                            Self::site(operation, DefaultOperationValueRoleV1::Result),
                        )?;
                    }
                }
                self.push_expression(pending, callback, depth)
            }
            crate::DefaultExpressionKindV1::FieldAccess { receiver, field } => {
                let principal =
                    self.validate_field_receiver(operation, receiver.result_type(), field)?;
                self.expect_result(expression, operation, &principal, true)?;
                self.push_expression(pending, receiver, depth)
            }
            crate::DefaultExpressionKindV1::MethodCall {
                receiver,
                callee,
                arguments,
            }
            | crate::DefaultExpressionKindV1::DirectSuperMethodCall {
                receiver,
                callee,
                arguments,
            } => {
                let shape = self.callable_shape(
                    DefaultOperationEntityV1::MethodCallee(callee),
                    operation,
                    DefaultOperationValueRoleV1::Callable,
                )?;
                self.ensure_no_captures(operation, &shape)?;
                self.ensure_callable_effect(
                    &shape,
                    Self::site(operation, DefaultOperationValueRoleV1::Callable),
                )?;
                let expected = shape.receiver().ok_or_else(|| {
                    ExportDefaultOperationTypingValidationError::Problem {
                        site: Self::site(operation, DefaultOperationValueRoleV1::Receiver),
                        problem: DefaultOperationTypingProblemV1::MissingCallableReceiver,
                    }
                })?;
                self.expect_relation(
                    DefaultOperationTypeRelationV1::MemberReceiver,
                    receiver.result_type(),
                    expected,
                    Self::site(operation, DefaultOperationValueRoleV1::Receiver),
                )?;
                self.expect_arguments(operation, arguments, shape.parameters())?;
                if matches!(
                    expression.kind(),
                    crate::DefaultExpressionKindV1::DirectSuperMethodCall { .. }
                ) {
                    self.validate_intrinsic(
                        DefaultOperationIntrinsicV1::DirectSuper(callee),
                        Self::site(operation, DefaultOperationValueRoleV1::Callable),
                    )?;
                }
                self.expect_result(expression, operation, shape.result(), true)?;
                self.push_expressions(pending, arguments, depth)?;
                self.push_expression(pending, receiver, depth)
            }
            crate::DefaultExpressionKindV1::Box(operand) => {
                self.expect_relation(
                    DefaultOperationTypeRelationV1::Boxing,
                    operand.result_type(),
                    expression.result_type(),
                    Self::site(operation, DefaultOperationValueRoleV1::Result),
                )?;
                self.push_expression(pending, operand, depth)
            }
            crate::DefaultExpressionKindV1::Unbox(operand) => {
                self.expect_relation(
                    DefaultOperationTypeRelationV1::Unboxing,
                    operand.result_type(),
                    expression.result_type(),
                    Self::site(operation, DefaultOperationValueRoleV1::Result),
                )?;
                self.push_expression(pending, operand, depth)
            }
            crate::DefaultExpressionKindV1::IsInstance {
                operand,
                checked_type,
            } => {
                self.expect_relation(
                    DefaultOperationTypeRelationV1::RuntimeTypeCheck,
                    operand.result_type(),
                    checked_type,
                    Self::site(operation, DefaultOperationValueRoleV1::CheckedType),
                )?;
                let boolean = self.core_type(
                    DefaultOperationCoreTypeV1::Boolean,
                    Self::site(operation, DefaultOperationValueRoleV1::Result),
                )?;
                self.expect_result(expression, operation, &boolean, false)?;
                self.push_expression(pending, operand, depth)
            }
            crate::DefaultExpressionKindV1::Cast { operand, optional } => {
                let target = if optional.value() {
                    self.core_application(
                        expression.result_type(),
                        DefaultOperationExpectedTypeShapeV1::Option,
                        Self::site(operation, DefaultOperationValueRoleV1::Result),
                    )?
                    .element()
                    .clone()
                } else {
                    expression.result_type().clone()
                };
                self.expect_relation(
                    DefaultOperationTypeRelationV1::RuntimeTypeCheck,
                    operand.result_type(),
                    &target,
                    Self::site(operation, DefaultOperationValueRoleV1::Target),
                )?;
                self.push_expression(pending, operand, depth)
            }
            crate::DefaultExpressionKindV1::ArrayLiteral(elements) => {
                let application = self.array_application(
                    expression.result_type(),
                    operation,
                    DefaultOperationValueRoleV1::Result,
                )?;
                self.expect_uniform_elements(operation, elements, application.element())?;
                self.push_expressions(pending, elements, depth)
            }
            crate::DefaultExpressionKindV1::ArrayAssembly(assembly) => {
                let application = self.core_application(
                    assembly.result_type(),
                    DefaultOperationExpectedTypeShapeV1::Array,
                    Self::site(operation, DefaultOperationValueRoleV1::EmbeddedResult),
                )?;
                self.expect_type(
                    application.element(),
                    assembly.element_type(),
                    Self::site(operation, DefaultOperationValueRoleV1::EmbeddedResult),
                )?;
                self.expect_result(expression, operation, assembly.result_type(), false)?;
                for (index, part) in assembly.parts().iter().enumerate() {
                    match part {
                        DefaultArrayAssemblyPartV1::Element(value) => self.expect_type(
                            value.result_type(),
                            assembly.element_type(),
                            Self::site(operation, DefaultOperationValueRoleV1::Part { index }),
                        )?,
                        DefaultArrayAssemblyPartV1::CopyArray(value) => {
                            let source = self.core_application(
                                value.result_type(),
                                DefaultOperationExpectedTypeShapeV1::Array,
                                Self::site(operation, DefaultOperationValueRoleV1::Part { index }),
                            )?;
                            self.expect_type(
                                source.element(),
                                assembly.element_type(),
                                Self::site(operation, DefaultOperationValueRoleV1::Part { index }),
                            )?;
                        }
                    }
                }
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
            crate::DefaultExpressionKindV1::Index {
                access,
                receiver,
                index,
            } => {
                let element =
                    self.array_access_element(*access, receiver.result_type(), false, operation)?;
                self.validate_array_index(operation, index)?;
                self.expect_result(expression, operation, &element, false)?;
                self.push_expression(pending, index, depth)?;
                self.push_expression(pending, receiver, depth)
            }
            crate::DefaultExpressionKindV1::ArraySet {
                access,
                receiver,
                index,
                value,
            } => {
                let element =
                    self.array_access_element(*access, receiver.result_type(), true, operation)?;
                self.validate_array_index(operation, index)?;
                self.expect_type(
                    value.result_type(),
                    &element,
                    Self::site(operation, DefaultOperationValueRoleV1::Value),
                )?;
                let unit = self.core_type(
                    DefaultOperationCoreTypeV1::Unit,
                    Self::site(operation, DefaultOperationValueRoleV1::Result),
                )?;
                self.expect_result(expression, operation, &unit, false)?;
                self.push_expression(pending, value, depth)?;
                self.push_expression(pending, index, depth)?;
                self.push_expression(pending, receiver, depth)
            }
            crate::DefaultExpressionKindV1::ArrayLen(operand) => {
                self.array_application(
                    operand.result_type(),
                    operation,
                    DefaultOperationValueRoleV1::Operand,
                )?;
                let long = self.integer_type(
                    DefaultIntegerKindV1::Signed64,
                    operation,
                    DefaultOperationValueRoleV1::Result,
                )?;
                self.expect_result(expression, operation, &long, false)?;
                self.push_expression(pending, operand, depth)
            }
            crate::DefaultExpressionKindV1::ArrayClone(operand) => {
                let source = self.array_application(
                    operand.result_type(),
                    operation,
                    DefaultOperationValueRoleV1::Operand,
                )?;
                let target = self.array_application(
                    expression.result_type(),
                    operation,
                    DefaultOperationValueRoleV1::Result,
                )?;
                self.expect_type(
                    source.element(),
                    target.element(),
                    Self::site(operation, DefaultOperationValueRoleV1::Result),
                )?;
                if matches!(
                    (&source, &target),
                    (
                        DefaultCoreApplicationV1::Array { .. },
                        DefaultCoreApplicationV1::MutableArray { .. }
                    ) | (
                        DefaultCoreApplicationV1::MutableArray { .. },
                        DefaultCoreApplicationV1::Array { .. }
                    )
                ) {
                    self.push_expression(pending, operand, depth)
                } else {
                    self.problem(
                        operation,
                        DefaultOperationValueRoleV1::Result,
                        DefaultOperationTypingProblemV1::InvalidArrayAccess,
                    )
                }
            }
            crate::DefaultExpressionKindV1::Call { callee, arguments } => {
                let shape = self.callable_shape(
                    DefaultOperationEntityV1::Callable(callee),
                    operation,
                    DefaultOperationValueRoleV1::Callable,
                )?;
                self.ensure_no_captures(operation, &shape)?;
                self.ensure_callable_effect(
                    &shape,
                    Self::site(operation, DefaultOperationValueRoleV1::Callable),
                )?;
                self.expect_direct_call_arguments(operation, arguments, &shape)?;
                self.expect_result(expression, operation, shape.result(), true)?;
                self.push_expressions(pending, arguments, depth)
            }
            crate::DefaultExpressionKindV1::LocalFunctionCall {
                declaration,
                callee,
                captures,
                arguments,
            } => {
                self.expect_local_declaration(operation, *declaration, callee.declaration())?;
                let shape = self.callable_shape(
                    DefaultOperationEntityV1::Callable(callee),
                    operation,
                    DefaultOperationValueRoleV1::Callable,
                )?;
                if shape.receiver().is_some() {
                    return self.problem(
                        operation,
                        DefaultOperationValueRoleV1::Receiver,
                        DefaultOperationTypingProblemV1::UnexpectedCallableReceiver,
                    );
                }
                self.ensure_callable_effect(
                    &shape,
                    Self::site(operation, DefaultOperationValueRoleV1::Callable),
                )?;
                self.expect_arguments_with_role(operation, captures, shape.captures(), |index| {
                    DefaultOperationValueRoleV1::Capture { index }
                })?;
                self.expect_arguments(operation, arguments, shape.parameters())?;
                self.expect_result(expression, operation, shape.result(), true)?;
                self.push_expressions(pending, arguments, depth)?;
                self.push_expressions(pending, captures, depth)
            }
            crate::DefaultExpressionKindV1::CallableCall {
                callee,
                function_type,
                arguments,
            } => {
                let (parameters, result, effect) = self
                    .expect_function(
                        function_type,
                        Self::site(operation, DefaultOperationValueRoleV1::Target),
                    )
                    .map(|(parameters, result, effect)| {
                        (parameters.to_vec(), result.clone(), effect)
                    })?;
                self.expect_type(
                    callee.result_type(),
                    function_type,
                    Self::site(operation, DefaultOperationValueRoleV1::Callable),
                )?;
                self.expect_arguments(operation, arguments, &parameters)?;
                if effect == Effect::Suspend && !self.template.allows_suspend().value() {
                    return self.problem(
                        operation,
                        DefaultOperationValueRoleV1::Callable,
                        DefaultOperationTypingProblemV1::SuspendNotAllowed,
                    );
                }
                self.expect_result(expression, operation, &result, true)?;
                self.push_expressions(pending, arguments, depth)?;
                self.push_expression(pending, callee, depth)
            }
            crate::DefaultExpressionKindV1::PrimitiveBinary { kind, lhs, rhs } => {
                let string = self.core_type(
                    DefaultOperationCoreTypeV1::String,
                    Self::site(operation, DefaultOperationValueRoleV1::Operand),
                )?;
                self.expect_type(
                    lhs.result_type(),
                    &string,
                    Self::site(
                        operation,
                        DefaultOperationValueRoleV1::Argument { index: 0 },
                    ),
                )?;
                self.expect_type(
                    rhs.result_type(),
                    &string,
                    Self::site(
                        operation,
                        DefaultOperationValueRoleV1::Argument { index: 1 },
                    ),
                )?;
                let principal = match kind {
                    DefaultPrimitiveBinaryKindV1::StringConcat => string,
                    DefaultPrimitiveBinaryKindV1::StringCompareTo => self.integer_type(
                        DefaultIntegerKindV1::Signed64,
                        operation,
                        DefaultOperationValueRoleV1::Result,
                    )?,
                };
                self.expect_result(expression, operation, &principal, false)?;
                self.push_expression(pending, rhs, depth)?;
                self.push_expression(pending, lhs, depth)
            }
            crate::DefaultExpressionKindV1::PrimitiveUnary { kind, operand } => {
                let boolean = match kind {
                    DefaultPrimitiveUnaryKindV1::BooleanNot => self.core_type(
                        DefaultOperationCoreTypeV1::Boolean,
                        Self::site(operation, DefaultOperationValueRoleV1::Operand),
                    )?,
                };
                self.expect_type(
                    operand.result_type(),
                    &boolean,
                    Self::site(operation, DefaultOperationValueRoleV1::Operand),
                )?;
                self.expect_result(expression, operation, &boolean, false)?;
                self.push_expression(pending, operand, depth)
            }
            crate::DefaultExpressionKindV1::IntegerOperation {
                operation: integer_operation,
                arguments,
            } => {
                self.validate_integer_operation(expression, integer_operation, arguments)?;
                self.push_integer_arguments(pending, arguments, depth)
            }
            crate::DefaultExpressionKindV1::IntegerConversion {
                source_kind,
                target_kind,
                target,
                operand,
            } => {
                let source_type = self.integer_type(
                    *source_kind,
                    operation,
                    DefaultOperationValueRoleV1::Operand,
                )?;
                let target_type = self.integer_type(
                    *target_kind,
                    operation,
                    DefaultOperationValueRoleV1::Result,
                )?;
                self.expect_type(
                    operand.result_type(),
                    &source_type,
                    Self::site(operation, DefaultOperationValueRoleV1::Operand),
                )?;
                let shape = self.callable_shape(
                    DefaultOperationEntityV1::Callable(target),
                    operation,
                    DefaultOperationValueRoleV1::Callable,
                )?;
                self.expect_integer_callable_shape(
                    operation,
                    &shape,
                    &source_type,
                    &[],
                    &target_type,
                )?;
                self.validate_intrinsic(
                    DefaultOperationIntrinsicV1::IntegerConversion {
                        source: *source_kind,
                        target: *target_kind,
                        callable: target,
                    },
                    Self::site(operation, DefaultOperationValueRoleV1::Callable),
                )?;
                self.expect_result(expression, operation, &target_type, false)?;
                self.push_expression(pending, operand, depth)
            }
            crate::DefaultExpressionKindV1::Binary { operator, lhs, rhs } => {
                let boolean = self.core_type(
                    DefaultOperationCoreTypeV1::Boolean,
                    Self::site(operation, DefaultOperationValueRoleV1::Result),
                )?;
                match operator {
                    DefaultBinaryOperatorV1::Lt
                    | DefaultBinaryOperatorV1::Le
                    | DefaultBinaryOperatorV1::Gt
                    | DefaultBinaryOperatorV1::Ge => {
                        let long = self.integer_type(
                            DefaultIntegerKindV1::Signed64,
                            operation,
                            DefaultOperationValueRoleV1::Operand,
                        )?;
                        self.expect_binary_types(operation, lhs, rhs, &long, &long)?;
                    }
                    DefaultBinaryOperatorV1::And | DefaultBinaryOperatorV1::Or => {
                        self.expect_binary_types(operation, lhs, rhs, &boolean, &boolean)?;
                    }
                    DefaultBinaryOperatorV1::RefEq | DefaultBinaryOperatorV1::RefNe => {
                        self.expect_relation(
                            DefaultOperationTypeRelationV1::ReferenceIdentity,
                            lhs.result_type(),
                            rhs.result_type(),
                            Self::site(operation, DefaultOperationValueRoleV1::Operand),
                        )?;
                    }
                }
                self.expect_result(expression, operation, &boolean, false)?;
                self.push_expression(pending, rhs, depth)?;
                self.push_expression(pending, lhs, depth)
            }
            crate::DefaultExpressionKindV1::Unary { operator, operand } => {
                let boolean = match operator {
                    DefaultUnaryOperatorV1::Not => self.core_type(
                        DefaultOperationCoreTypeV1::Boolean,
                        Self::site(operation, DefaultOperationValueRoleV1::Operand),
                    )?,
                };
                self.expect_type(
                    operand.result_type(),
                    &boolean,
                    Self::site(operation, DefaultOperationValueRoleV1::Operand),
                )?;
                self.expect_result(expression, operation, &boolean, false)?;
                self.push_expression(pending, operand, depth)
            }
            crate::DefaultExpressionKindV1::SomeWrap(operand) => {
                let option = self.core_application(
                    expression.result_type(),
                    DefaultOperationExpectedTypeShapeV1::Option,
                    Self::site(operation, DefaultOperationValueRoleV1::Result),
                )?;
                self.expect_type(
                    operand.result_type(),
                    option.element(),
                    Self::site(operation, DefaultOperationValueRoleV1::Operand),
                )?;
                self.push_expression(pending, operand, depth)
            }
            crate::DefaultExpressionKindV1::NoneLiteral => {
                self.core_application(
                    expression.result_type(),
                    DefaultOperationExpectedTypeShapeV1::Option,
                    Self::site(operation, DefaultOperationValueRoleV1::Result),
                )?;
                Ok(())
            }
            crate::DefaultExpressionKindV1::IsSome(operand) => {
                self.core_application(
                    operand.result_type(),
                    DefaultOperationExpectedTypeShapeV1::Option,
                    Self::site(operation, DefaultOperationValueRoleV1::Operand),
                )?;
                let boolean = self.core_type(
                    DefaultOperationCoreTypeV1::Boolean,
                    Self::site(operation, DefaultOperationValueRoleV1::Result),
                )?;
                self.expect_result(expression, operation, &boolean, false)?;
                self.push_expression(pending, operand, depth)
            }
            crate::DefaultExpressionKindV1::Unwrap { operand, .. } => {
                let option = self.core_application(
                    operand.result_type(),
                    DefaultOperationExpectedTypeShapeV1::Option,
                    Self::site(operation, DefaultOperationValueRoleV1::Operand),
                )?;
                self.expect_result(expression, operation, option.element(), false)?;
                self.push_expression(pending, operand, depth)
            }
        }
    }

    fn integer_type(
        &mut self,
        kind: DefaultIntegerKindV1,
        operation: DefaultExpressionOperationV1,
        role: DefaultOperationValueRoleV1,
    ) -> Result<SignatureTypeKey, ExportDefaultOperationTypingValidationError<E>> {
        self.core_type(
            DefaultOperationCoreTypeV1::Integer(kind),
            Self::site(operation, role),
        )
    }

    fn constructor_shape(
        &mut self,
        constructor: &crate::DefaultConstructorRefV1,
        operation: DefaultExpressionOperationV1,
    ) -> Result<
        (SignatureTypeKey, Vec<SignatureTypeKey>),
        ExportDefaultOperationTypingValidationError<E>,
    > {
        let site = Self::site(operation, DefaultOperationValueRoleV1::Target);
        let shape = self.entity_shape(DefaultOperationEntityV1::Constructor(constructor), site)?;
        match shape {
            DefaultOperationEntityShapeV1::Constructor {
                owner_type,
                parameters,
            } => Ok((owner_type, parameters)),
            actual => Err(ExportDefaultOperationTypingValidationError::EntityShape {
                site,
                expected: DefaultOperationEntityShapeKindV1::Constructor,
                actual: actual.kind(),
            }),
        }
    }

    fn aggregate_shape(
        &mut self,
        entity: DefaultOperationEntityV1<'_>,
        operation: DefaultExpressionOperationV1,
        role: DefaultOperationValueRoleV1,
    ) -> Result<
        crate::DefaultAggregateOperationShapeV1,
        ExportDefaultOperationTypingValidationError<E>,
    > {
        let site = Self::site(operation, role);
        let shape = self.entity_shape(entity, site)?;
        match shape {
            DefaultOperationEntityShapeV1::Aggregate(shape) => Ok(shape),
            actual => Err(ExportDefaultOperationTypingValidationError::EntityShape {
                site,
                expected: DefaultOperationEntityShapeKindV1::Aggregate,
                actual: actual.kind(),
            }),
        }
    }

    fn variant_field_shape(
        &mut self,
        field: &crate::DefaultEnumVariantFieldRefV1,
        operation: DefaultExpressionOperationV1,
    ) -> Result<
        crate::DefaultVariantFieldOperationShapeV1,
        ExportDefaultOperationTypingValidationError<E>,
    > {
        let site = Self::site(operation, DefaultOperationValueRoleV1::Target);
        let shape = self.entity_shape(DefaultOperationEntityV1::VariantField(field), site)?;
        match shape {
            DefaultOperationEntityShapeV1::VariantField(shape) => Ok(shape),
            actual => Err(ExportDefaultOperationTypingValidationError::EntityShape {
                site,
                expected: DefaultOperationEntityShapeKindV1::VariantField,
                actual: actual.kind(),
            }),
        }
    }

    fn value_shape(
        &mut self,
        entity: DefaultOperationEntityV1<'_>,
        operation: DefaultExpressionOperationV1,
        role: DefaultOperationValueRoleV1,
    ) -> Result<crate::DefaultValueOperationShapeV1, ExportDefaultOperationTypingValidationError<E>>
    {
        let site = Self::site(operation, role);
        let shape = self.entity_shape(entity, site)?;
        match shape {
            DefaultOperationEntityShapeV1::Value(shape) => Ok(shape),
            actual => Err(ExportDefaultOperationTypingValidationError::EntityShape {
                site,
                expected: DefaultOperationEntityShapeKindV1::Value,
                actual: actual.kind(),
            }),
        }
    }

    fn type_shape(
        &mut self,
        entity: DefaultOperationEntityV1<'_>,
        operation: DefaultExpressionOperationV1,
        role: DefaultOperationValueRoleV1,
    ) -> Result<SignatureTypeKey, ExportDefaultOperationTypingValidationError<E>> {
        let site = Self::site(operation, role);
        let shape = self.entity_shape(entity, site)?;
        match shape {
            DefaultOperationEntityShapeV1::Type(shape) => Ok(shape),
            actual => Err(ExportDefaultOperationTypingValidationError::EntityShape {
                site,
                expected: DefaultOperationEntityShapeKindV1::Type,
                actual: actual.kind(),
            }),
        }
    }

    fn callback_shape(
        &mut self,
        registration: scoop_identity::PersistentCallbackRegistrationId,
        operation: DefaultExpressionOperationV1,
    ) -> Result<
        crate::DefaultCallbackOperationShapeV1,
        ExportDefaultOperationTypingValidationError<E>,
    > {
        let site = Self::site(operation, DefaultOperationValueRoleV1::Target);
        let shape = self.entity_shape(
            DefaultOperationEntityV1::CallbackRegistration(registration),
            site,
        )?;
        match shape {
            DefaultOperationEntityShapeV1::CallbackRegistration(shape) => Ok(shape),
            actual => Err(ExportDefaultOperationTypingValidationError::EntityShape {
                site,
                expected: DefaultOperationEntityShapeKindV1::CallbackRegistration,
                actual: actual.kind(),
            }),
        }
    }

    fn callable_shape(
        &mut self,
        entity: DefaultOperationEntityV1<'_>,
        operation: DefaultExpressionOperationV1,
        role: DefaultOperationValueRoleV1,
    ) -> Result<DefaultCallableOperationShapeV1, ExportDefaultOperationTypingValidationError<E>>
    {
        let site = Self::site(operation, role);
        let shape = self.entity_shape(entity, site)?;
        match shape {
            DefaultOperationEntityShapeV1::Callable(shape) => Ok(shape),
            actual => Err(ExportDefaultOperationTypingValidationError::EntityShape {
                site,
                expected: DefaultOperationEntityShapeKindV1::Callable,
                actual: actual.kind(),
            }),
        }
    }

    fn validate_nested_callable_type(
        &mut self,
        expression: &crate::DefaultExpressionV1,
        operation: DefaultExpressionOperationV1,
        function_type: &SignatureTypeKey,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        self.expect_function(
            function_type,
            Self::site(operation, DefaultOperationValueRoleV1::Target),
        )?;
        self.expect_result(expression, operation, function_type, true)
    }

    fn validate_optional_offset(
        &mut self,
        operation: DefaultExpressionOperationV1,
        offset: Option<&crate::DefaultExpressionV1>,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        let Some(offset) = offset else {
            return Ok(());
        };
        let long = self.integer_type(
            DefaultIntegerKindV1::Signed64,
            operation,
            DefaultOperationValueRoleV1::Offset,
        )?;
        self.expect_type(
            offset.result_type(),
            &long,
            Self::site(operation, DefaultOperationValueRoleV1::Offset),
        )
    }

    fn place_type(
        &mut self,
        place: &DefaultPlaceV1,
        operation: DefaultExpressionOperationV1,
    ) -> Result<SignatureTypeKey, ExportDefaultOperationTypingValidationError<E>> {
        match place {
            DefaultPlaceV1::Local { local } => {
                self.charge_work()?;
                self.template
                    .locals()
                    .get(local)
                    .map(|record| record.value_type().clone())
                    .ok_or_else(|| ExportDefaultOperationTypingValidationError::Problem {
                        site: Self::site(operation, DefaultOperationValueRoleV1::Place),
                        problem: DefaultOperationTypingProblemV1::MissingLocal,
                    })
            }
            DefaultPlaceV1::Global { property } => self
                .value_shape(
                    DefaultOperationEntityV1::Global(*property),
                    operation,
                    DefaultOperationValueRoleV1::Place,
                )
                .map(|shape| shape.value_type().clone()),
        }
    }

    fn validate_field_receiver(
        &mut self,
        operation: DefaultExpressionOperationV1,
        receiver: &SignatureTypeKey,
        field: &crate::DefaultFieldRefV1,
    ) -> Result<SignatureTypeKey, ExportDefaultOperationTypingValidationError<E>> {
        if let crate::DefaultFieldRefV1::Tuple { declaration_index } = field {
            self.charge_work()?;
            let SignatureTypeKey::Tuple(elements) = receiver else {
                return Err(ExportDefaultOperationTypingValidationError::TypeShape {
                    site: Self::site(operation, DefaultOperationValueRoleV1::Receiver),
                    expected: DefaultOperationExpectedTypeShapeV1::Tuple,
                    actual: Box::new(receiver.clone()),
                });
            };
            return elements
                .as_slice()
                .get(*declaration_index as usize)
                .cloned()
                .ok_or(ExportDefaultOperationTypingValidationError::Arity {
                    site: Self::site(operation, DefaultOperationValueRoleV1::Target),
                    expected: elements.as_slice().len(),
                    actual: (*declaration_index as usize).saturating_add(1),
                });
        }

        let site = Self::site(operation, DefaultOperationValueRoleV1::Target);
        let shape = self.entity_shape(DefaultOperationEntityV1::Field(field), site)?;
        let DefaultOperationEntityShapeV1::Field(shape) = shape else {
            return Err(ExportDefaultOperationTypingValidationError::EntityShape {
                site,
                expected: DefaultOperationEntityShapeKindV1::Field,
                actual: shape.kind(),
            });
        };
        let (expected_kind, encoded_owner) = match field {
            crate::DefaultFieldRefV1::Struct { owner_type, .. } => {
                (DefaultFieldOperationKindV1::Struct, owner_type)
            }
            crate::DefaultFieldRefV1::Class { owner_type, .. } => {
                (DefaultFieldOperationKindV1::Class, owner_type)
            }
            crate::DefaultFieldRefV1::Tuple { .. } => unreachable!("handled above"),
        };
        if shape.kind() != expected_kind {
            return self.problem(
                operation,
                DefaultOperationValueRoleV1::Target,
                DefaultOperationTypingProblemV1::FieldKindMismatch {
                    expected: expected_kind,
                    actual: shape.kind(),
                },
            );
        }
        self.expect_type(encoded_owner, shape.owner_type(), site)?;
        match expected_kind {
            DefaultFieldOperationKindV1::Struct => self.expect_type(
                receiver,
                shape.owner_type(),
                Self::site(operation, DefaultOperationValueRoleV1::Receiver),
            )?,
            DefaultFieldOperationKindV1::Class => self.expect_relation(
                DefaultOperationTypeRelationV1::MemberReceiver,
                receiver,
                shape.owner_type(),
                Self::site(operation, DefaultOperationValueRoleV1::Receiver),
            )?,
        }
        Ok(shape.value_type().clone())
    }

    fn array_application(
        &mut self,
        value: &SignatureTypeKey,
        operation: DefaultExpressionOperationV1,
        role: DefaultOperationValueRoleV1,
    ) -> Result<DefaultCoreApplicationV1, ExportDefaultOperationTypingValidationError<E>> {
        let site = Self::site(operation, role);
        self.charge_work()?;
        let actual = self
            .authority
            .classify_default_core_application(value, self.meter, self.path)
            .map_err(
                |error| ExportDefaultOperationTypingValidationError::Authority { site, error },
            )?;
        match actual {
            Some(
                application @ (DefaultCoreApplicationV1::Array { .. }
                | DefaultCoreApplicationV1::MutableArray { .. }),
            ) => Ok(application),
            actual => Err(
                ExportDefaultOperationTypingValidationError::CoreApplication {
                    site,
                    expected: DefaultOperationExpectedTypeShapeV1::Array,
                    actual: actual.as_ref().map(DefaultCoreApplicationV1::kind),
                },
            ),
        }
    }

    fn array_access_element(
        &mut self,
        access: DefaultArrayAccessKindV1,
        receiver: &SignatureTypeKey,
        store: bool,
        operation: DefaultExpressionOperationV1,
    ) -> Result<SignatureTypeKey, ExportDefaultOperationTypingValidationError<E>> {
        let application =
            self.array_application(receiver, operation, DefaultOperationValueRoleV1::Receiver)?;
        let valid = matches!(
            (store, access, &application),
            (
                false,
                DefaultArrayAccessKindV1::ImmutableGet,
                DefaultCoreApplicationV1::Array { .. }
            ) | (
                false,
                DefaultArrayAccessKindV1::MutableGet,
                DefaultCoreApplicationV1::MutableArray { .. }
            ) | (
                true,
                DefaultArrayAccessKindV1::MutableSet,
                DefaultCoreApplicationV1::MutableArray { .. }
            )
        );
        if !valid {
            return self.problem(
                operation,
                DefaultOperationValueRoleV1::Receiver,
                DefaultOperationTypingProblemV1::InvalidArrayAccess,
            );
        }
        Ok(application.element().clone())
    }

    fn validate_array_index(
        &mut self,
        operation: DefaultExpressionOperationV1,
        index: &crate::DefaultExpressionV1,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        let long = self.integer_type(
            DefaultIntegerKindV1::Signed64,
            operation,
            DefaultOperationValueRoleV1::Index,
        )?;
        self.expect_type(
            index.result_type(),
            &long,
            Self::site(operation, DefaultOperationValueRoleV1::Index),
        )
    }

    fn expect_uniform_elements(
        &mut self,
        operation: DefaultExpressionOperationV1,
        elements: &[crate::DefaultExpressionV1],
        expected: &SignatureTypeKey,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        for (index, element) in elements.iter().enumerate() {
            self.expect_type(
                element.result_type(),
                expected,
                Self::site(operation, DefaultOperationValueRoleV1::Argument { index }),
            )?;
        }
        Ok(())
    }

    fn expect_arguments(
        &mut self,
        operation: DefaultExpressionOperationV1,
        actual: &[crate::DefaultExpressionV1],
        expected: &[SignatureTypeKey],
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        self.expect_arguments_with_role(operation, actual, expected, |index| {
            DefaultOperationValueRoleV1::Argument { index }
        })
    }

    fn expect_arguments_with_role(
        &mut self,
        operation: DefaultExpressionOperationV1,
        actual: &[crate::DefaultExpressionV1],
        expected: &[SignatureTypeKey],
        role: impl Fn(usize) -> DefaultOperationValueRoleV1,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        self.expect_arity(
            actual.len(),
            expected.len(),
            Self::site(operation, DefaultOperationValueRoleV1::Callable),
        )?;
        for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
            self.expect_type(
                actual.result_type(),
                expected,
                Self::site(operation, role(index)),
            )?;
        }
        Ok(())
    }

    fn expect_direct_call_arguments(
        &mut self,
        operation: DefaultExpressionOperationV1,
        actual: &[crate::DefaultExpressionV1],
        shape: &DefaultCallableOperationShapeV1,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        let receiver_count = usize::from(shape.receiver().is_some());
        self.expect_arity(
            actual.len(),
            receiver_count + shape.parameters().len(),
            Self::site(operation, DefaultOperationValueRoleV1::Callable),
        )?;
        if let Some(receiver) = shape.receiver() {
            self.expect_type(
                actual[0].result_type(),
                receiver,
                Self::site(operation, DefaultOperationValueRoleV1::Receiver),
            )?;
        }
        for (index, (actual, expected)) in actual[receiver_count..]
            .iter()
            .zip(shape.parameters())
            .enumerate()
        {
            self.expect_type(
                actual.result_type(),
                expected,
                Self::site(operation, DefaultOperationValueRoleV1::Argument { index }),
            )?;
        }
        Ok(())
    }

    fn ensure_no_captures(
        &mut self,
        operation: DefaultExpressionOperationV1,
        shape: &DefaultCallableOperationShapeV1,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        self.charge_work()?;
        if shape.captures().is_empty() {
            Ok(())
        } else {
            self.problem(
                operation,
                DefaultOperationValueRoleV1::Callable,
                DefaultOperationTypingProblemV1::UnexpectedCallableCaptures,
            )
        }
    }

    fn expect_local_declaration(
        &mut self,
        operation: DefaultExpressionOperationV1,
        declaration: CallableTemplateOrigin,
        callee: DefaultCallableDeclarationV1,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        self.charge_work()?;
        let matches = match (declaration, callee) {
            (
                CallableTemplateOrigin::Function(expected),
                DefaultCallableDeclarationV1::Function(actual),
            ) => expected == actual,
            (
                CallableTemplateOrigin::GenericFunction(expected),
                DefaultCallableDeclarationV1::GenericFunction(actual),
            ) => expected == actual,
            _ => false,
        };
        if matches {
            Ok(())
        } else {
            self.problem(
                operation,
                DefaultOperationValueRoleV1::Callable,
                DefaultOperationTypingProblemV1::LocalDeclarationMismatch,
            )
        }
    }

    fn expect_callable_reference_type(
        &mut self,
        operation: DefaultExpressionOperationV1,
        shape: &DefaultCallableOperationShapeV1,
        include_receiver: bool,
        target: &SignatureTypeKey,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        self.expect_function(
            target,
            Self::site(operation, DefaultOperationValueRoleV1::Target),
        )?;
        let receiver_count = usize::from(include_receiver && shape.receiver().is_some());
        let mut parameters = Vec::new();
        self.meter
            .try_reserve_collection_slots(
                &mut parameters,
                receiver_count + shape.parameters().len(),
                self.path,
            )
            .map_err(ExportDefaultOperationTypingValidationError::Resource)?;
        if include_receiver && let Some(receiver) = shape.receiver() {
            parameters.push(receiver.clone());
        }
        parameters.extend_from_slice(shape.parameters());
        let source = SignatureTypeKey::Function {
            effect: shape.effect(),
            parameters,
            result: Box::new(shape.result().clone()),
        };
        if &source == target {
            self.charge_work()
        } else {
            self.expect_relation(
                DefaultOperationTypeRelationV1::FunctionCoercion,
                &source,
                target,
                Self::site(operation, DefaultOperationValueRoleV1::Target),
            )
        }
    }

    fn validate_integer_operation(
        &mut self,
        expression: &crate::DefaultExpressionV1,
        integer_operation: &DefaultIntegerOperationV1,
        arguments: &DefaultIntegerArgumentsV1,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        let operation = DefaultExpressionOperationV1::IntegerOperation;
        let (kind, target, rhs, result) = match integer_operation {
            DefaultIntegerOperationV1::NoGc {
                kind,
                operation: intrinsic,
                target,
            } => {
                let integer =
                    self.integer_type(*kind, operation, DefaultOperationValueRoleV1::Operand)?;
                let (rhs, result) = match intrinsic {
                    DefaultNoGcIntegerOperationV1::UnaryPlus
                    | DefaultNoGcIntegerOperationV1::UnaryMinus
                    | DefaultNoGcIntegerOperationV1::Inc
                    | DefaultNoGcIntegerOperationV1::Dec
                    | DefaultNoGcIntegerOperationV1::Inv => (None, integer.clone()),
                    DefaultNoGcIntegerOperationV1::Add
                    | DefaultNoGcIntegerOperationV1::Sub
                    | DefaultNoGcIntegerOperationV1::Mul
                    | DefaultNoGcIntegerOperationV1::And
                    | DefaultNoGcIntegerOperationV1::Or
                    | DefaultNoGcIntegerOperationV1::Xor => {
                        (Some(integer.clone()), integer.clone())
                    }
                    DefaultNoGcIntegerOperationV1::CompareTo => (
                        Some(integer.clone()),
                        self.integer_type(
                            DefaultIntegerKindV1::Signed64,
                            operation,
                            DefaultOperationValueRoleV1::Result,
                        )?,
                    ),
                    DefaultNoGcIntegerOperationV1::Equals => (
                        Some(integer.clone()),
                        self.core_type(
                            DefaultOperationCoreTypeV1::Boolean,
                            Self::site(operation, DefaultOperationValueRoleV1::Result),
                        )?,
                    ),
                    DefaultNoGcIntegerOperationV1::Shl
                    | DefaultNoGcIntegerOperationV1::Shr
                    | DefaultNoGcIntegerOperationV1::Ushr => (
                        Some(self.integer_type(
                            DefaultIntegerKindV1::Signed64,
                            operation,
                            DefaultOperationValueRoleV1::Argument { index: 1 },
                        )?),
                        integer.clone(),
                    ),
                };
                (*kind, target, rhs, result)
            }
            DefaultIntegerOperationV1::Managed { kind, target, .. } => {
                let integer =
                    self.integer_type(*kind, operation, DefaultOperationValueRoleV1::Operand)?;
                (*kind, target, Some(integer.clone()), integer)
            }
        };
        let integer = self.integer_type(kind, operation, DefaultOperationValueRoleV1::Operand)?;
        match (arguments, rhs.as_ref()) {
            (DefaultIntegerArgumentsV1::Unary(operand), None) => self.expect_type(
                operand.result_type(),
                &integer,
                Self::site(operation, DefaultOperationValueRoleV1::Operand),
            )?,
            (DefaultIntegerArgumentsV1::Binary { lhs, rhs }, Some(rhs_type)) => {
                self.expect_type(
                    lhs.result_type(),
                    &integer,
                    Self::site(
                        operation,
                        DefaultOperationValueRoleV1::Argument { index: 0 },
                    ),
                )?;
                self.expect_type(
                    rhs.result_type(),
                    rhs_type,
                    Self::site(
                        operation,
                        DefaultOperationValueRoleV1::Argument { index: 1 },
                    ),
                )?;
            }
            (DefaultIntegerArgumentsV1::Unary(_), Some(_)) => {
                return Err(ExportDefaultOperationTypingValidationError::Arity {
                    site: Self::site(operation, DefaultOperationValueRoleV1::Callable),
                    expected: 2,
                    actual: 1,
                });
            }
            (DefaultIntegerArgumentsV1::Binary { .. }, None) => {
                return Err(ExportDefaultOperationTypingValidationError::Arity {
                    site: Self::site(operation, DefaultOperationValueRoleV1::Callable),
                    expected: 1,
                    actual: 2,
                });
            }
        }
        let shape = self.callable_shape(
            DefaultOperationEntityV1::Callable(target),
            operation,
            DefaultOperationValueRoleV1::Callable,
        )?;
        let parameters = rhs.as_slice();
        self.expect_integer_callable_shape(operation, &shape, &integer, parameters, &result)?;
        self.validate_intrinsic(
            DefaultOperationIntrinsicV1::IntegerOperation(integer_operation),
            Self::site(operation, DefaultOperationValueRoleV1::Callable),
        )?;
        self.expect_result(expression, operation, &result, false)
    }

    fn expect_integer_callable_shape(
        &mut self,
        operation: DefaultExpressionOperationV1,
        shape: &DefaultCallableOperationShapeV1,
        receiver: &SignatureTypeKey,
        parameters: &[SignatureTypeKey],
        result: &SignatureTypeKey,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        self.ensure_no_captures(operation, shape)?;
        self.ensure_callable_effect(
            shape,
            Self::site(operation, DefaultOperationValueRoleV1::Callable),
        )?;
        let actual_receiver = shape.receiver().ok_or_else(|| {
            ExportDefaultOperationTypingValidationError::Problem {
                site: Self::site(operation, DefaultOperationValueRoleV1::Receiver),
                problem: DefaultOperationTypingProblemV1::MissingCallableReceiver,
            }
        })?;
        self.expect_type(
            actual_receiver,
            receiver,
            Self::site(operation, DefaultOperationValueRoleV1::Receiver),
        )?;
        self.expect_arity(
            shape.parameters().len(),
            parameters.len(),
            Self::site(operation, DefaultOperationValueRoleV1::Callable),
        )?;
        for (index, (actual, expected)) in shape.parameters().iter().zip(parameters).enumerate() {
            self.expect_type(
                actual,
                expected,
                Self::site(operation, DefaultOperationValueRoleV1::Argument { index }),
            )?;
        }
        self.expect_type(
            shape.result(),
            result,
            Self::site(operation, DefaultOperationValueRoleV1::Result),
        )
    }

    fn expect_binary_types(
        &mut self,
        operation: DefaultExpressionOperationV1,
        lhs: &crate::DefaultExpressionV1,
        rhs: &crate::DefaultExpressionV1,
        lhs_type: &SignatureTypeKey,
        rhs_type: &SignatureTypeKey,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        self.expect_type(
            lhs.result_type(),
            lhs_type,
            Self::site(
                operation,
                DefaultOperationValueRoleV1::Argument { index: 0 },
            ),
        )?;
        self.expect_type(
            rhs.result_type(),
            rhs_type,
            Self::site(
                operation,
                DefaultOperationValueRoleV1::Argument { index: 1 },
            ),
        )
    }

    fn push_integer_arguments<'body>(
        &mut self,
        pending: &mut Vec<ExpressionWork<'body>>,
        arguments: &'body DefaultIntegerArgumentsV1,
        depth: u64,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        match arguments {
            DefaultIntegerArgumentsV1::Unary(operand) => {
                self.push_expression(pending, operand, depth)
            }
            DefaultIntegerArgumentsV1::Binary { lhs, rhs } => {
                self.push_expression(pending, rhs, depth)?;
                self.push_expression(pending, lhs, depth)
            }
        }
    }

    fn push_expression<'body>(
        &mut self,
        pending: &mut Vec<ExpressionWork<'body>>,
        expression: &'body crate::DefaultExpressionV1,
        parent_depth: u64,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        let depth = self.child_depth(parent_depth)?;
        self.meter
            .try_reserve_collection_slots(pending, 1, self.path)
            .map_err(ExportDefaultOperationTypingValidationError::Resource)?;
        pending.push(ExpressionWork { expression, depth });
        Ok(())
    }

    fn push_expressions<'body>(
        &mut self,
        pending: &mut Vec<ExpressionWork<'body>>,
        expressions: &'body [crate::DefaultExpressionV1],
        parent_depth: u64,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        for expression in expressions.iter().rev() {
            self.push_expression(pending, expression, parent_depth)?;
        }
        Ok(())
    }

    fn problem<T>(
        &self,
        operation: DefaultExpressionOperationV1,
        role: DefaultOperationValueRoleV1,
        problem: DefaultOperationTypingProblemV1,
    ) -> Result<T, ExportDefaultOperationTypingValidationError<E>> {
        Err(ExportDefaultOperationTypingValidationError::Problem {
            site: Self::site(operation, role),
            problem,
        })
    }
}

struct ExpressionWork<'a> {
    expression: &'a crate::DefaultExpressionV1,
    depth: u64,
}
