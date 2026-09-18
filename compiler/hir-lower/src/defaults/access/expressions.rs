use scoop_hir as hir;

use super::ReferenceCollector;

impl ReferenceCollector<'_> {
    pub(super) fn expression(&mut self, expression: &hir::Expr) {
        let origin = expression.origin.definition();
        self.type_reference(expression.ty, origin);
        match &expression.kind {
            hir::ExprKind::StringLiteral { .. }
            | hir::ExprKind::IntegerLiteral(_)
            | hir::ExprKind::BoolLiteral(_)
            | hir::ExprKind::UnitLiteral
            | hir::ExprKind::ConstructorParam(_)
            | hir::ExprKind::Local(_)
            | hir::ExprKind::Capture(_)
            | hir::ExprKind::NoneLiteral => {}
            hir::ExprKind::InitializingClassFieldAccess { .. }
            | hir::ExprKind::InitializingStructFieldAccess { .. } => {}
            hir::ExprKind::TupleLiteral(values) | hir::ExprKind::ArrayLiteral(values) => {
                self.expressions(values);
            }
            hir::ExprKind::StructInit { constructor, args } => {
                self.constructor_use(
                    hir::ExportDefaultConstructorTarget::Struct(*constructor),
                    origin,
                );
                self.expressions(args);
            }
            hir::ExprKind::StructConstruct {
                application,
                fields,
            } => {
                let ty = self.lowerer.struct_applications[*application].canonical_type;
                self.type_reference(ty, origin);
                self.expressions(fields);
            }
            hir::ExprKind::ClassInit { constructor, args } => {
                self.constructor_use(
                    hir::ExportDefaultConstructorTarget::Class(*constructor),
                    origin,
                );
                self.expressions(args);
            }
            hir::ExprKind::VariantConstruct { variant, args } => {
                self.constructor_use(
                    hir::ExportDefaultConstructorTarget::Variant(*variant),
                    origin,
                );
                self.expressions(args);
            }
            hir::ExprKind::VariantTest { operand, variant } => {
                self.expression(operand);
                self.constructor_use(
                    hir::ExportDefaultConstructorTarget::Variant(*variant),
                    origin,
                );
            }
            hir::ExprKind::VariantPayloadProject { operand, field } => {
                self.expression(operand);
                self.variant_field_shape(*field, origin);
            }
            hir::ExprKind::GlobalRead(global) => self.global(*global, origin),
            hir::ExprKind::SingletonValue(value) => self.singleton_value(*value, origin),
            hir::ExprKind::Lambda(lambda) => self.lambda_descriptor(*lambda, origin),
            hir::ExprKind::AnonymousFunction(function) => {
                self.anonymous_function_descriptor(*function, origin);
            }
            hir::ExprKind::CallableReference(reference) => {
                self.callable_reference_descriptor(*reference, origin);
            }
            hir::ExprKind::FunctionCoercion {
                source,
                coercion,
                target_type,
            } => {
                let source_type = self.lowerer.function_coercions[*coercion].source;
                let source_type = self.lowerer.function_types[source_type].canonical_type;
                let target_type = self.lowerer.function_types[*target_type].canonical_type;
                self.type_reference(source_type, origin);
                self.type_reference(target_type, origin);
                self.expression(source);
            }
            hir::ExprKind::PtrFromNonZeroULong(source)
            | hir::ExprKind::PtrToULong(source)
            | hir::ExprKind::PtrCast(source)
            | hir::ExprKind::Box(source)
            | hir::ExprKind::Unbox(source)
            | hir::ExprKind::ArrayLen(source)
            | hir::ExprKind::ArrayClone(source)
            | hir::ExprKind::SomeWrap(source)
            | hir::ExprKind::IsSome(source) => self.expression(source),
            hir::ExprKind::PtrLoad { pointer, offset } => {
                self.expression(pointer);
                if let Some(offset) = offset {
                    self.expression(offset);
                }
            }
            hir::ExprKind::PtrStore {
                pointer,
                offset,
                value,
            } => {
                self.expression(pointer);
                if let Some(offset) = offset {
                    self.expression(offset);
                }
                self.expression(value);
            }
            hir::ExprKind::PtrOffset {
                pointer, offset, ..
            }
            | hir::ExprKind::Index {
                receiver: pointer,
                index: offset,
                ..
            }
            | hir::ExprKind::PrimitiveBinary {
                lhs: pointer,
                rhs: offset,
                ..
            }
            | hir::ExprKind::Binary {
                lhs: pointer,
                rhs: offset,
                ..
            } => {
                self.expression(pointer);
                self.expression(offset);
            }
            hir::ExprKind::ArraySet {
                receiver,
                index,
                value,
                ..
            } => {
                self.expression(receiver);
                self.expression(index);
                self.expression(value);
            }
            hir::ExprKind::AddressOf(place) => {
                if let hir::Place::Global(global) = place {
                    self.global(*global, origin);
                }
            }
            hir::ExprKind::SizeOf(ty) | hir::ExprKind::AlignOf(ty) => {
                self.type_reference(*ty, origin);
            }
            hir::ExprKind::FunctionAddress(function) => self.record_callable(
                hir::ExportDefaultCallableTarget::FunctionAddress(*function),
                origin,
            ),
            hir::ExprKind::ForeignCallbackRegister { closure, .. } => self.expression(closure),
            hir::ExprKind::ForeignCallbackOperation { callback, .. } => self.expression(callback),
            hir::ExprKind::FieldAccess { receiver, field } => {
                self.expression(receiver);
                self.field_use(*field, origin);
            }
            hir::ExprKind::MethodCall {
                receiver,
                callee,
                args,
            }
            | hir::ExprKind::DirectSuperMethodCall {
                receiver,
                callee,
                args,
            } => {
                self.expression(receiver);
                self.method_callee_use(*callee, origin);
                self.expressions(args);
            }
            hir::ExprKind::IsInstance { operand, check_ty } => {
                self.expression(operand);
                self.type_reference(*check_ty, origin);
            }
            hir::ExprKind::Cast { operand, .. }
            | hir::ExprKind::Unary { operand, .. }
            | hir::ExprKind::PrimitiveUnary { operand, .. }
            | hir::ExprKind::Unwrap { operand, .. } => self.expression(operand),
            hir::ExprKind::ArrayAssembly(assembly) => {
                let result_type =
                    self.lowerer.class_applications[assembly.result_type].canonical_type;
                self.type_reference(result_type, origin);
                self.type_reference(assembly.element_type, origin);
                for part in &assembly.parts {
                    match part {
                        hir::ArrayAssemblyPart::Element(value)
                        | hir::ArrayAssemblyPart::CopyArray(value) => self.expression(value),
                    }
                }
            }
            hir::ExprKind::Call { callee, args } => {
                self.callable_use(*callee, origin);
                self.expressions(args);
            }
            hir::ExprKind::ImportedCoreCall { callee, args } => {
                self.record_callable(
                    hir::ExportDefaultCallableTarget::ImportedCore(*callee),
                    origin,
                );
                self.expressions(args);
            }
            hir::ExprKind::ImportedDependencyCall { callee, args } => {
                self.record_callable(
                    hir::ExportDefaultCallableTarget::ImportedDependency(*callee),
                    origin,
                );
                self.expressions(args);
            }
            hir::ExprKind::LocalFunctionCall {
                local_function,
                callee,
                captures,
                args,
            } => {
                self.record_callable(
                    hir::ExportDefaultCallableTarget::LocalFunction(*local_function),
                    origin,
                );
                self.callable_use(*callee, origin);
                self.expressions(captures);
                self.expressions(args);
            }
            hir::ExprKind::CallableCall {
                callee,
                function_type,
                args,
            } => {
                let function_type = self.lowerer.function_types[*function_type].canonical_type;
                self.type_reference(function_type, origin);
                self.expression(callee);
                self.expressions(args);
            }
            hir::ExprKind::IntegerOperation {
                operation,
                arguments,
            } => {
                let function = match operation {
                    hir::IntegerOperation::NoGc { target, .. } => target.function(),
                    hir::IntegerOperation::Managed { target, .. } => target.function(),
                };
                self.callable_use(hir::Callable::Function(function), origin);
                match arguments {
                    hir::HirIntegerOperationArguments::Unary(operand) => self.expression(operand),
                    hir::HirIntegerOperationArguments::Binary { lhs, rhs } => {
                        self.expression(lhs);
                        self.expression(rhs);
                    }
                }
            }
            hir::ExprKind::IntegerConversion {
                conversion,
                operand,
            } => {
                self.callable_use(
                    hir::Callable::Function(conversion.target.function()),
                    origin,
                );
                self.expression(operand);
            }
        }
    }

    pub(super) fn expressions(&mut self, expressions: &[hir::Expr]) {
        for expression in expressions {
            self.expression(expression);
        }
    }
}
