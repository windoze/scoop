use scoop_hir as hir;

use super::ReferenceCollector;

impl ReferenceCollector<'_> {
    pub(super) fn reference_target(
        &mut self,
        target: &hir::CallableReferenceTarget,
        origin: hir::DefinitionOrigin,
    ) {
        if let hir::CallableReferenceTarget::BoundMember { callee, .. } = target {
            self.method_callee_shape(*callee, origin);
        } else if let Some(callee) = target.callee(&self.lowerer.bound_callable_refs) {
            self.callable_target_shape(callee, origin);
        }
        if let Some(receiver) = target.receiver() {
            self.expression(receiver);
        }
    }

    pub(super) fn callable_target(
        &mut self,
        callee: hir::CallableTarget,
        origin: hir::DefinitionOrigin,
    ) {
        self.callable_target_shape(callee, origin);
        let target = match callee {
            hir::CallableTarget::Local(callable) => {
                hir::ExportDefaultCallableTarget::Callable(callable)
            }
            hir::CallableTarget::Application(application) => {
                hir::ExportDefaultCallableTarget::ImportedGeneric(application)
            }
            hir::CallableTarget::Dependency(callee) => {
                hir::ExportDefaultCallableTarget::ImportedDependency(callee)
            }
        };
        self.record_callable(target, origin);
    }

    fn direct_call_target(
        &mut self,
        callee: hir::CallableTarget,
        result_type: hir::TypeId,
        origin: hir::DefinitionOrigin,
    ) {
        if let hir::CallableTarget::Local(callable) = callee {
            let function = self.lowerer.callable_function_id(callable);
            if let Some(&local) = self.lowerer.local_function_by_function.get(&function) {
                self.record_callable(
                    hir::ExportDefaultCallableTarget::LocalFunction(local),
                    origin,
                );
            }
        }
        if let hir::CallableTarget::Dependency(callee) = callee {
            let reference = self.lowerer.imported_dependency_callables[callee].reference();
            let selected = self
                .lowerer
                .dependencies
                .as_ref()
                .and_then(|dependencies| dependencies.resolve_callable(reference))
                .expect("a call retains its selected declaration");
            if let scoop_identity::CallableTemplateOrigin::Constructor(declaration) =
                selected.interface().declaration()
            {
                self.constructor_use(
                    hir::ExportDefaultConstructorTarget::Imported {
                        declaration,
                        owner_type: result_type,
                    },
                    origin,
                );
                return;
            }
        }
        self.callable_target(callee, origin);
    }

    pub(super) fn callable_target_shape(
        &mut self,
        callee: hir::CallableTarget,
        origin: hir::DefinitionOrigin,
    ) {
        let application = match callee {
            hir::CallableTarget::Local(callable) => return self.callable_shape(callable, origin),
            hir::CallableTarget::Application(application) => application,
            hir::CallableTarget::Dependency(_) => return,
        };
        let arguments = self.lowerer.imported_generic_applications[application]
            .arguments
            .clone();
        let arguments = match arguments {
            hir::ImportedCallableArguments::Function(arguments) => arguments.to_vec(),
            hir::ImportedCallableArguments::Method {
                owner,
                method_arguments,
            } => {
                self.type_reference(owner, origin);
                method_arguments
            }
        };
        for ty in arguments {
            self.type_reference(ty, origin);
        }
    }

    pub(super) fn expression(&mut self, expression: &hir::Expr) {
        let origin = expression.origin.definition();
        self.type_reference(expression.ty, origin);
        match &expression.kind {
            hir::ExprKind::StringLiteral { .. }
            | hir::ExprKind::IntegerLiteral(_)
            | hir::ExprKind::BoolLiteral(_)
            | hir::ExprKind::UnitLiteral
            | hir::ExprKind::ConstructorReceiver
            | hir::ExprKind::ConstructorParam(_)
            | hir::ExprKind::Local(_)
            | hir::ExprKind::Capture(_)
            | hir::ExprKind::NoneLiteral => {}
            hir::ExprKind::InitializingClassFieldAccess { .. }
            | hir::ExprKind::InitializingStructFieldAccess { .. } => {}
            hir::ExprKind::ReleaseFieldLoad(field) => self.field_use(
                hir::FieldRef::ClassField {
                    owner: field.owner,
                    field: field.field,
                },
                origin,
            ),
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
                let owner = variant.owner;
                self.type_reference(owner, origin);
            }
            hir::ExprKind::VariantPayloadProject { operand, field } => {
                self.expression(operand);
                self.variant_field_shape(*field, origin);
            }
            hir::ExprKind::GlobalRead(global) => self.global(*global, origin),
            hir::ExprKind::GenericDelegateStorageRead(_) => self.direct_delegate_storage(origin),
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
                self.expression(source);
                self.type_reference(source_type, origin);
                self.type_reference(target_type, origin);
            }
            hir::ExprKind::PtrFromNonZeroULong(source)
            | hir::ExprKind::PtrToULong(source)
            | hir::ExprKind::PtrCast(source)
            | hir::ExprKind::Box(source)
            | hir::ExprKind::Unbox(source)
            | hir::ExprKind::ReferenceUpcast(source)
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
            hir::ExprKind::AddressOf(place) => match place {
                hir::Place::Global(global) => self.global(*global, origin),
                hir::Place::ExternalGlobal { property, .. } => {
                    self.external_global(*property, origin)
                }
                hir::Place::Local(_) => {}
            },
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
            hir::ExprKind::IsInstance { operand, check_ty }
            | hir::ExprKind::Cast {
                operand, check_ty, ..
            } => {
                self.expression(operand);
                self.type_reference(*check_ty, origin);
            }
            hir::ExprKind::Unary { operand, .. }
            | hir::ExprKind::PrimitiveUnary { operand, .. }
            | hir::ExprKind::Unwrap { operand, .. } => self.expression(operand),
            hir::ExprKind::ArrayAssembly(assembly) => {
                let result_type = assembly.result_type;
                self.type_reference(assembly.element_type, origin);
                for part in &assembly.parts {
                    match part {
                        hir::ArrayAssemblyPart::Element(value)
                        | hir::ArrayAssemblyPart::CopyArray(value) => self.expression(value),
                    }
                }
                self.type_reference(result_type, origin);
            }
            hir::ExprKind::Call {
                callee,
                args,
                receiver,
                ..
            } => {
                self.direct_call_target(*callee, expression.ty, origin);
                self.expressions(args);
                if let hir::SourceCallReceiver::Receiver { static_type } = receiver {
                    self.type_reference(*static_type, origin);
                }
            }
            hir::ExprKind::CallableCall {
                callee,
                function_type,
                args,
            } => {
                let function_type = self.lowerer.function_types[*function_type].canonical_type;
                self.expression(callee);
                self.type_reference(function_type, origin);
                self.expressions(args);
            }
            hir::ExprKind::IntegerOperation { arguments, .. } => match arguments {
                hir::HirIntegerOperationArguments::Unary(operand) => self.expression(operand),
                hir::HirIntegerOperationArguments::Binary { lhs, rhs } => {
                    self.expression(lhs);
                    self.expression(rhs);
                }
            },
            hir::ExprKind::IntegerConversion { operand, .. } => self.expression(operand),
        }
    }

    pub(super) fn expressions(&mut self, expressions: &[hir::Expr]) {
        for expression in expressions {
            self.expression(expression);
        }
    }
}
