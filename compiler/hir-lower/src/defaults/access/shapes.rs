use scoop_hir as hir;

use super::ReferenceCollector;

impl ReferenceCollector<'_> {
    pub(super) fn callable_use(&mut self, callable: hir::Callable, origin: hir::DefinitionOrigin) {
        self.callable_shape(callable, origin);
        self.record_callable(hir::ExportDefaultCallableTarget::Callable(callable), origin);
    }

    pub(super) fn method_callee_use(
        &mut self,
        callee: hir::MethodCallee,
        origin: hir::DefinitionOrigin,
    ) {
        let target = match callee {
            hir::MethodCallee::Callable(callable) => return self.callable_target(callable, origin),
            hir::MethodCallee::Bound(bound) => hir::ExportDefaultCallableTarget::Bound(bound),
            hir::MethodCallee::DerivedEquality(application) => {
                hir::ExportDefaultCallableTarget::DerivedEquality(application)
            }
        };
        self.method_callee_shape(callee, origin);
        self.record_callable(target, origin);
    }

    pub(super) fn callable_shape(
        &mut self,
        callable: hir::Callable,
        origin: hir::DefinitionOrigin,
    ) {
        let (owner, arguments) = match callable {
            hir::Callable::Function(_) => (None, Vec::new()),
            hir::Callable::Generic(id) => {
                let application = self.lowerer.instantiations[id].clone();
                (None, application.type_args)
            }
            hir::Callable::Method(id) => {
                let application = self.lowerer.method_applications[id].clone();
                (Some(self.method_owner_type(application.owner)), Vec::new())
            }
            hir::Callable::GenericMethod(id) => {
                let application = self.lowerer.generic_method_applications[id].clone();
                (
                    Some(self.generic_method_owner_type(application.owner)),
                    application.method_arguments.to_vec(),
                )
            }
        };
        if let Some(owner) = owner {
            self.type_reference(owner, origin);
        }
        for argument in arguments {
            self.type_reference(argument, origin);
        }
    }

    pub(super) fn method_callee_shape(
        &mut self,
        callee: hir::MethodCallee,
        origin: hir::DefinitionOrigin,
    ) {
        match callee {
            hir::MethodCallee::Callable(callable) => self.callable_target_shape(callable, origin),
            hir::MethodCallee::Bound(bound) => self.bound_callable_shape(bound, origin),
            hir::MethodCallee::DerivedEquality(application) => {
                let owner = self.lowerer.derived_equality_applications[application].owner_ty;
                self.type_reference(owner, origin);
            }
        }
    }

    fn bound_callable_shape(&mut self, id: hir::BoundCallableRefId, origin: hir::DefinitionOrigin) {
        let bound = self.lowerer.bound_callable_refs[id].clone();
        let signature = self.lowerer.function_types[bound.instantiated_signature].canonical_type;
        self.type_reference(bound.receiver_type, origin);
        match bound.source {
            hir::BoundCallableSource::Class { bound, callable } => {
                let ty = self.lowerer.class_applications[bound].canonical_type;
                self.type_reference(ty, origin);
                self.callable_target_shape(callable, origin);
            }
            hir::BoundCallableSource::Interface {
                bound, declared, ..
            } => {
                let ty = self.lowerer.interface_applications[bound].canonical_type;
                self.type_reference(ty, origin);
                self.callable_target_shape(declared, origin);
            }
        }
        self.type_reference(signature, origin);
    }

    pub(super) fn constructor_use(
        &mut self,
        target: hir::ExportDefaultConstructorTarget,
        origin: hir::DefinitionOrigin,
    ) {
        let owner = match target {
            hir::ExportDefaultConstructorTarget::Imported { owner_type, .. } => owner_type,
            hir::ExportDefaultConstructorTarget::Struct(id) => {
                let owner = self.lowerer.struct_constructor_applications[id].owner;
                self.lowerer.struct_applications[owner].canonical_type
            }
            hir::ExportDefaultConstructorTarget::Class(id) => {
                let owner = self.lowerer.class_constructor_applications[id].owner;
                self.lowerer.class_applications[owner].canonical_type
            }
            hir::ExportDefaultConstructorTarget::Variant(variant) => variant.owner,
        };
        self.type_reference(owner, origin);
        self.record_constructor(target, origin);
    }

    pub(super) fn field_use(&mut self, field: hir::FieldRef, origin: hir::DefinitionOrigin) {
        let owner = match field {
            hir::FieldRef::StructField { owner, .. } | hir::FieldRef::ClassField { owner, .. } => {
                Some(owner)
            }
            hir::FieldRef::TupleIndex(_) => None,
        };
        if let Some(owner) = owner {
            self.type_reference(owner, origin);
        }
        self.record_field(field, origin);
    }

    pub(super) fn variant_field_shape(
        &mut self,
        field: hir::EnumVariantFieldApplication,
        origin: hir::DefinitionOrigin,
    ) {
        let owner = field.variant.owner;
        self.type_reference(owner, origin);
    }

    pub(super) fn lambda_descriptor(&mut self, id: hir::LambdaId, origin: hir::DefinitionOrigin) {
        let lambda = self.lowerer.lambdas[id].clone();
        self.lexical_callable_shape(
            lambda.function_type,
            &lambda.body_type_arguments,
            &lambda.captures,
            origin,
        );
        self.record_callable(hir::ExportDefaultCallableTarget::Lambda(id), origin);
    }

    pub(super) fn anonymous_function_descriptor(
        &mut self,
        id: hir::AnonymousFunctionId,
        origin: hir::DefinitionOrigin,
    ) {
        let function = self.lowerer.anonymous_functions[id].clone();
        self.lexical_callable_shape(
            function.function_type,
            &function.body_type_arguments,
            &function.captures,
            origin,
        );
        self.record_callable(
            hir::ExportDefaultCallableTarget::AnonymousFunction(id),
            origin,
        );
    }

    pub(super) fn local_function_descriptor(&mut self, id: hir::LocalFunctionId) {
        let function = self.lowerer.local_functions[id].clone();
        let origin = function.origin;
        self.local_declarations.insert(function.function);
        let ty = self.lowerer.function_types[function.function_type].canonical_type;
        let domain = self.lowerer.type_access_domain(ty);
        let target_domain = self.checked_target_domain(domain, origin, "a type");
        self.references.types.push(hir::ExportDefaultTypeRef {
            target: hir::ExportDefaultTypeTarget::LocalFunctionSignature(id),
            target_domain,
            origin,
        });
        self.capture_shapes(&function.captures);
        self.record_callable(hir::ExportDefaultCallableTarget::LocalFunction(id), origin);
    }

    pub(super) fn callable_reference_descriptor(
        &mut self,
        id: hir::CallableReferenceId,
        origin: hir::DefinitionOrigin,
    ) {
        let reference = self.lowerer.callable_references[id].clone();
        self.record_callable(
            hir::ExportDefaultCallableTarget::CallableReference(id),
            origin,
        );
        self.function_type_reference(reference.function_type, origin);
        match reference.target {
            hir::CallableReferenceTarget::Imported(target) => {
                self.imported_reference_target(&target, origin);
            }
            hir::CallableReferenceTarget::Named(callable)
            | hir::CallableReferenceTarget::Local {
                callee: callable, ..
            } => self.callable_shape(callable, origin),
            hir::CallableReferenceTarget::BoundMember { receiver, callee } => {
                self.expression(&receiver);
                self.method_callee_shape(callee, origin);
            }
            hir::CallableReferenceTarget::BoundExtension { receiver, callee } => {
                self.expression(&receiver);
                self.callable_shape(callee, origin);
            }
        }
        self.capture_shapes(&reference.captures);
    }

    fn lexical_callable_shape(
        &mut self,
        function_type: hir::FunctionTypeId,
        body_arguments: &hir::CallableBodyTypeArguments,
        captures: &[hir::Capture],
        origin: hir::DefinitionOrigin,
    ) {
        self.function_type_reference(function_type, origin);
        if let hir::CallableBodyTypeArguments::Explicit(arguments) = body_arguments {
            for &argument in arguments {
                self.type_reference(argument, origin);
            }
        }
        self.capture_shapes(captures);
    }

    fn capture_shapes(&mut self, captures: &[hir::Capture]) {
        for capture in captures {
            let mut origin = capture.source.origin.definition();
            origin.span = capture.first_use_span;
            self.type_reference(capture.ty, origin);
        }
    }

    pub(super) fn function_type_reference(
        &mut self,
        function_type: hir::FunctionTypeId,
        origin: hir::DefinitionOrigin,
    ) {
        let ty = self.lowerer.function_types[function_type].canonical_type;
        self.type_reference(ty, origin);
    }

    fn method_owner_type(&self, owner: hir::MethodOwnerApplication) -> hir::TypeId {
        match owner {
            hir::MethodOwnerApplication::Class(id) => {
                self.lowerer.class_applications[id].canonical_type
            }
            hir::MethodOwnerApplication::Struct(id) => {
                self.lowerer.struct_applications[id].canonical_type
            }
            hir::MethodOwnerApplication::Enum(id) => {
                self.lowerer.enum_applications[id].canonical_type
            }
            hir::MethodOwnerApplication::Interface(id) => {
                self.lowerer.interface_applications[id].canonical_type
            }
            hir::MethodOwnerApplication::Object(id) => self.lowerer.object_types[id].canonical_type,
        }
    }

    fn generic_method_owner_type(&self, owner: hir::GenericMethodOwner) -> hir::TypeId {
        match owner {
            hir::GenericMethodOwner::Class(id) => {
                self.lowerer.class_applications[id].canonical_type
            }
            hir::GenericMethodOwner::Struct(id) => {
                self.lowerer.struct_applications[id].canonical_type
            }
            hir::GenericMethodOwner::Enum(id) => self.lowerer.enum_applications[id].canonical_type,
            hir::GenericMethodOwner::Object(id) => self.lowerer.object_types[id].canonical_type,
        }
    }
}
