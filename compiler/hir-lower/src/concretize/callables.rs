use super::*;

impl Concretizer<'_> {
    pub(super) fn lower_callable(
        &mut self,
        source: export::Callable,
        substitution: &[concrete::TypeId],
    ) -> concrete::Callable {
        self.lower_callable_with_arguments(source, substitution).0
    }

    pub(super) fn adapt_method_receiver(
        &mut self,
        receiver: concrete::Expr,
        target: concrete::TypeId,
    ) -> concrete::Expr {
        if receiver.ty == target {
            return receiver;
        }
        let span = receiver.span;
        let origin = receiver.origin;
        let value = matches!(
            self.types[receiver.ty].kind,
            concrete::TypeKind::Unit
                | concrete::TypeKind::Integer(_)
                | concrete::TypeKind::Boolean
                | concrete::TypeKind::Struct(_)
                | concrete::TypeKind::Enum(_)
                | concrete::TypeKind::Tuple(_)
                | concrete::TypeKind::Ptr(_)
                | concrete::TypeKind::FunPtr(_)
        );
        if value {
            self.ensure_box_source(receiver.ty);
            concrete::Expr {
                kind: concrete::ExprKind::Box(Box::new(receiver)),
                ty: target,
                span,
                origin,
            }
        } else {
            concrete::Expr {
                kind: concrete::ExprKind::ReferenceUpcast(Box::new(receiver)),
                ty: target,
                span,
                origin,
            }
        }
    }

    pub(super) fn lower_callable_with_arguments(
        &mut self,
        source: export::Callable,
        substitution: &[concrete::TypeId],
    ) -> (concrete::Callable, Vec<concrete::TypeId>) {
        match source {
            export::Callable::Function(function) => {
                assert!(
                    self.source.functions[function].type_param_count() == 0,
                    "a parameterized callable uses a resolved generic identity"
                );
                (
                    concrete::Callable::Function(self.request_function(function, Vec::new())),
                    Vec::new(),
                )
            }
            export::Callable::Generic(resolved) => {
                let resolved = self.source.instantiations[resolved].clone();
                let function = self.source.generic_functions[resolved.generic].function;
                let arguments: Vec<_> = resolved
                    .type_args
                    .iter()
                    .map(|argument| self.lower_type(*argument, substitution))
                    .collect();
                (
                    concrete::Callable::Function(
                        self.request_function(function, arguments.clone()),
                    ),
                    arguments,
                )
            }
            export::Callable::Method(application) => {
                self.lower_method_application_with_arguments(application, substitution)
            }
            export::Callable::GenericMethod(application) => {
                let application = self.source.generic_method_applications[application].clone();
                let owner = self.lower_generic_method_owner(application.owner, substitution);
                let method_arguments = export::NonEmptyVec::from_vec(
                    application
                        .method_arguments
                        .iter()
                        .map(|argument| self.lower_type(*argument, substitution))
                        .collect(),
                )
                .expect("export generic method applications are non-empty");
                let mut arguments = self.concrete_method_owner_arguments(owner).to_vec();
                arguments.extend(method_arguments.iter().copied());
                let function = self.source.generic_methods[application.method].function;
                let concrete = self.request_method(
                    function,
                    owner,
                    method_arguments.iter().copied().collect(),
                );
                (concrete::Callable::Function(concrete), arguments)
            }
        }
    }

    pub(super) fn lower_method_application(
        &mut self,
        source: export::MethodApplicationId,
        substitution: &[concrete::TypeId],
    ) -> concrete::Callable {
        self.lower_method_application_with_arguments(source, substitution)
            .0
    }

    pub(super) fn lower_method_application_with_arguments(
        &mut self,
        source: export::MethodApplicationId,
        substitution: &[concrete::TypeId],
    ) -> (concrete::Callable, Vec<concrete::TypeId>) {
        let application = self.source.method_applications[source].clone();
        let owner = self.lower_method_owner(application.owner, substitution);
        let arguments = self.concrete_method_owner_arguments(owner).to_vec();
        let function = self.request_method(application.function, owner, Vec::new());
        (concrete::Callable::Function(function), arguments)
    }

    pub(super) fn lower_method_owner(
        &mut self,
        owner: export::MethodOwnerApplication,
        substitution: &[concrete::TypeId],
    ) -> concrete::MethodOwner {
        match owner {
            export::MethodOwnerApplication::Class(id) => {
                concrete::MethodOwner::Class(self.lower_class_application(id, substitution))
            }
            export::MethodOwnerApplication::Struct(id) => {
                concrete::MethodOwner::Struct(self.lower_struct_application(id, substitution))
            }
            export::MethodOwnerApplication::Enum(id) => {
                concrete::MethodOwner::Enum(self.lower_enum_application(id, substitution))
            }
            export::MethodOwnerApplication::Interface(id) => {
                concrete::MethodOwner::Interface(self.lower_interface_application(id, substitution))
            }
            export::MethodOwnerApplication::Object(id) => {
                concrete::MethodOwner::Object(self.lower_object_type(id))
            }
        }
    }

    pub(super) fn lower_generic_method_owner(
        &mut self,
        owner: export::GenericMethodOwner,
        substitution: &[concrete::TypeId],
    ) -> concrete::MethodOwner {
        match owner {
            export::GenericMethodOwner::Class(id) => {
                concrete::MethodOwner::Class(self.lower_class_application(id, substitution))
            }
            export::GenericMethodOwner::Struct(id) => {
                concrete::MethodOwner::Struct(self.lower_struct_application(id, substitution))
            }
            export::GenericMethodOwner::Enum(id) => {
                concrete::MethodOwner::Enum(self.lower_enum_application(id, substitution))
            }
            export::GenericMethodOwner::Object(id) => {
                concrete::MethodOwner::Object(self.lower_object_type(id))
            }
        }
    }

    pub(super) fn lower_place(
        &mut self,
        source: &export::Place,
        locals: &[concrete::LocalId],
        substitution: &[concrete::TypeId],
        span: export::Span,
    ) -> concrete::Place {
        match source {
            export::Place::Local(local) => concrete::Place::Local(self.lower_local(*local, locals)),
            export::Place::Global(global) => concrete::Place::Global(self.global_map[global]),
            export::Place::ExternalGlobal {
                property,
                source_contract,
                ty,
            } => {
                let ty = self.lower_type(*ty, substitution);
                concrete::Place::Global(self.lower_external_global(
                    *property,
                    source_contract,
                    ty,
                    span,
                ))
            }
        }
    }

    pub(super) fn lower_field_ref(
        &mut self,
        source: export::FieldRef,
        substitution: &[concrete::TypeId],
    ) -> concrete::FieldRef {
        match source {
            export::FieldRef::ClassField { owner, field } => {
                let ty = self.lower_type(owner, substitution);
                let concrete::TypeKind::Class(class_id) = self.types[ty].kind else {
                    unreachable!("a class field retains its declaring class")
                };
                self.class_field_ref(class_id, field)
            }
            export::FieldRef::StructField { owner, field } => {
                let ty = self.lower_type(owner, substitution);
                let concrete::TypeKind::Struct(structure) = self.types[ty].kind else {
                    unreachable!("a struct field retains its declaring struct")
                };
                let index = self.structs[structure]
                    .declared_fields()
                    .iter()
                    .position(|candidate| candidate.identity == field)
                    .expect("a resolved field belongs to its declaration");
                concrete::FieldRef::StructField(
                    concrete::StructFieldRef::checked(&self.structs, structure, index as u32)
                        .expect("a declared field is in range"),
                )
            }
            export::FieldRef::TupleIndex(index) => concrete::FieldRef::TupleIndex(index),
        }
    }

    pub(super) fn lower_local(
        &self,
        source: export::LocalId,
        locals: &[concrete::LocalId],
    ) -> concrete::LocalId {
        locals[source.into_raw().into_u32() as usize]
    }

    pub(super) fn lower_capture(
        &mut self,
        source: &export::Capture,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::Capture {
        concrete::Capture {
            binding: concrete::BindingId::from_raw(source.binding.into_raw()),
            name: source.name.clone(),
            ty: self.lower_type(source.ty, substitution),
            first_use_span: source.first_use_span,
            source: self.lower_expr(&source.source, substitution, locals),
        }
    }
}
