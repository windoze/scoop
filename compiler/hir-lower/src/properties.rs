use scoop_ast as ast;
use scoop_hir as hir;

use crate::{Lowerer, Owner, TypeId};

mod declarations;
mod delegates;

pub(crate) use delegates::DelegateRoleCall;

#[derive(Clone)]
pub(crate) struct PropertyAccessorSource {
    pub(crate) property: hir::PropertyId,
    pub(crate) kind: PropertyAccessorKind,
    pub(crate) function: hir::FunctionId,
    pub(crate) declaration: ast::FunctionDecl,
    pub(crate) owner: Option<Owner>,
    pub(crate) backing: Option<hir::ClassFieldId>,
    pub(crate) generated_delegate: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PropertyAccessorKind {
    Getter,
    Setter,
}

#[derive(Clone, Copy)]
pub(crate) struct LocalDelegatePlan {
    pub(crate) property_ty: TypeId,
    pub(crate) mutable: bool,
    pub(crate) getter: LocalDelegateAccessor,
    pub(crate) setter: Option<LocalDelegateAccessor>,
}

#[derive(Clone, Copy)]
pub(crate) struct LocalDelegateAccessor {
    pub(crate) dispatch: LocalDelegateDispatch,
    pub(crate) effect: hir::Callable,
    pub(crate) result_ty: TypeId,
}

#[derive(Clone, Copy)]
pub(crate) enum LocalDelegateDispatch {
    Member(hir::MethodCallee),
    Extension(hir::Callable),
}

#[derive(Clone)]
pub(crate) struct BackingFieldContext {
    pub(crate) field: hir::ClassFieldId,
    pub(crate) capture_depth: usize,
}

#[derive(Clone)]
pub(crate) struct ResolvedExtensionProperty {
    pub(crate) property: hir::PropertyId,
    pub(crate) receiver: hir::Expr,
    pub(crate) type_args: Vec<TypeId>,
    pub(crate) read: hir::Expr,
}

pub(crate) enum ExtensionPropertyResolution {
    NoCandidate,
    Failed,
    Resolved(Box<ResolvedExtensionProperty>),
}

impl Lowerer {
    pub(crate) fn resolve_extension_property(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        sink: &mut Vec<hir::Statement>,
        require_read: bool,
    ) -> ExtensionPropertyResolution {
        let declared = self
            .extension_properties_by_name
            .get(&name.text)
            .is_some_and(|properties| !properties.is_empty());
        let mut first_failure = None;
        for same_side in [true, false] {
            let mut state = self.clone();
            let mut layer_sink = Vec::new();
            match state.resolve_extension_property_on_side(
                receiver.clone(),
                name,
                same_side,
                &mut layer_sink,
                require_read,
            ) {
                ExtensionPropertyResolution::Resolved(resolved) => {
                    *self = state;
                    sink.extend(layer_sink);
                    return ExtensionPropertyResolution::Resolved(resolved);
                }
                ExtensionPropertyResolution::Failed => {
                    first_failure.get_or_insert(Box::new(state));
                }
                ExtensionPropertyResolution::NoCandidate => {}
            }
        }
        if let Some(failure) = first_failure {
            self.commit_layer_diagnostics(*failure);
            ExtensionPropertyResolution::Failed
        } else if declared {
            self.error(
                name.span,
                format!("extension property `{}` is not accessible here", name.text),
            );
            ExtensionPropertyResolution::Failed
        } else {
            ExtensionPropertyResolution::NoCandidate
        }
    }

    pub(crate) fn resolve_extension_property_on_side(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        same_side: bool,
        sink: &mut Vec<hir::Statement>,
        require_read: bool,
    ) -> ExtensionPropertyResolution {
        let call_site_is_core = self.current_file < self.user_file_index;
        let properties = self
            .extension_properties_by_name
            .get(&name.text)
            .into_iter()
            .flatten()
            .copied()
            .filter(|property| {
                self.access_domain_allows(
                    &self.properties[*property].access.lookup.0,
                    Some(receiver.ty),
                )
            })
            .filter(|property| {
                ((self.property_files[property] < self.user_file_index) == call_site_is_core)
                    == same_side
            })
            .collect::<Vec<_>>();
        if properties.is_empty() {
            return ExtensionPropertyResolution::NoCandidate;
        }
        let getters = properties
            .iter()
            .map(|property| {
                match self.property_getters[self.properties[*property].capability.getter()]
                    .implementation
                {
                    hir::PropertyAccessorImplementation::Body(function) => function,
                    hir::PropertyAccessorImplementation::Storage
                    | hir::PropertyAccessorImplementation::Constant
                    | hir::PropertyAccessorImplementation::AbstractSlot(_) => {
                        unreachable!("extension properties have concrete getter bodies")
                    }
                }
            })
            .collect::<Vec<_>>();
        let no_type_args = [];
        let no_arguments = [];
        let Some(resolved) = self.resolve_extension_overload(
            &name.text,
            &getters,
            receiver,
            crate::overload::OverloadCall {
                explicit_type_args: &no_type_args,
                arg_exprs: &no_arguments,
                span: name.span,
                expected_result: None,
                argument_protocol: crate::overload::CallArgumentProtocol::Ordinary,
            },
            sink,
        ) else {
            return ExtensionPropertyResolution::Failed;
        };
        let function = resolved.function();
        let property = self.extension_property_by_getter[&function];
        let receiver = resolved
            .args
            .first()
            .cloned()
            .expect("an extension getter materializes its receiver argument");
        let callee = self.materialize_resolved_callee(&resolved);
        if require_read {
            self.check_call_effects(callee, name.span);
        }
        let read = hir::Expr {
            kind: hir::ExprKind::Call {
                callee,
                args: resolved.args,
            },
            ty: resolved.return_ty,
            span: name.span,
            origin: self.expression_origin(name.span),
        };
        ExtensionPropertyResolution::Resolved(Box::new(ResolvedExtensionProperty {
            property,
            receiver,
            type_args: resolved.type_args,
            read,
        }))
    }

    pub(crate) fn lower_extension_property_write(
        &mut self,
        resolved: ResolvedExtensionProperty,
        value: hir::Expr,
        span: ast::Span,
    ) -> Option<hir::StatementKind> {
        let property = self.properties[resolved.property].clone();
        let Some(setter) = property.capability.setter() else {
            self.error(
                span,
                format!("cannot assign to immutable property `{}`", property.name),
            );
            return None;
        };
        let setter = self.property_setters[setter].clone();
        if !self.access_domain_allows(&setter.access.lookup.0, Some(resolved.receiver.ty)) {
            self.error(
                span,
                format!("setter of property `{}` is not accessible", property.name),
            );
            return None;
        }
        let function = match setter.implementation {
            hir::PropertyAccessorImplementation::Body(function) => function,
            hir::PropertyAccessorImplementation::Storage
            | hir::PropertyAccessorImplementation::Constant
            | hir::PropertyAccessorImplementation::AbstractSlot(_) => {
                unreachable!("extension properties use concrete accessor functions")
            }
        };
        let candidate = crate::CallableCandidate::function(
            function,
            Vec::new(),
            self.function_lookup_witness(function),
        );
        let callee = self.materialize_candidate_callable(&candidate, &resolved.type_args);
        self.check_call_effects(callee, span);
        Some(hir::StatementKind::Expr(hir::Expr {
            kind: hir::ExprKind::Call {
                callee,
                args: vec![resolved.receiver, value],
            },
            ty: self.unit,
            span,
            origin: self.expression_origin(span),
        }))
    }

    pub(crate) fn find_accessible_nominal_property(
        &mut self,
        receiver_ty: TypeId,
        name: &str,
    ) -> Option<(hir::PropertyId, hir::MethodOwnerApplication, TypeId)> {
        match self.types[receiver_ty].clone() {
            hir::Type::Class(application) => {
                let (declaring, property, ty) = self.find_accessible_class_application_property(
                    application,
                    name,
                    receiver_ty,
                )?;
                Some((property, hir::MethodOwnerApplication::Class(declaring), ty))
            }
            hir::Type::Struct(application) => {
                let value = self.struct_applications[application].clone();
                let property = self.structs[value.template]
                    .properties
                    .iter()
                    .copied()
                    .find(|&property| {
                        self.properties[property].name == name
                            && self.access_domain_allows(
                                &self.properties[property].access.lookup.0,
                                Some(receiver_ty),
                            )
                    })?;
                let ty = self.instantiate_ty(self.properties[property].ty, &value.arguments);
                Some((
                    property,
                    hir::MethodOwnerApplication::Struct(application),
                    ty,
                ))
            }
            hir::Type::Enum(application) => {
                let value = self.enum_applications[application].clone();
                let property =
                    self.enums[value.template]
                        .properties
                        .iter()
                        .copied()
                        .find(|&property| {
                            self.properties[property].name == name
                                && self.access_domain_allows(
                                    &self.properties[property].access.lookup.0,
                                    Some(receiver_ty),
                                )
                        })?;
                let ty = self.instantiate_ty(self.properties[property].ty, &value.arguments);
                Some((property, hir::MethodOwnerApplication::Enum(application), ty))
            }
            hir::Type::Interface(application) => {
                let (property, declaring, ty) = self
                    .find_accessible_interface_application_property(
                        application,
                        name,
                        receiver_ty,
                        &mut Vec::new(),
                    )?;
                Some((
                    property,
                    hir::MethodOwnerApplication::Interface(declaring),
                    ty,
                ))
            }
            _ => None,
        }
    }

    pub(crate) fn find_accessible_interface_application_property(
        &mut self,
        application: hir::InterfaceApplicationId,
        name: &str,
        receiver_ty: TypeId,
        seen: &mut Vec<hir::InterfaceApplicationId>,
    ) -> Option<(hir::PropertyId, hir::InterfaceApplicationId, TypeId)> {
        if seen.contains(&application) {
            return None;
        }
        seen.push(application);
        let value = self.interface_applications[application].clone();
        if let Some(property) = self.interfaces[value.template]
            .properties
            .iter()
            .copied()
            .find(|&property| {
                self.properties[property].name == name
                    && self.access_domain_allows(
                        &self.properties[property].access.lookup.0,
                        Some(receiver_ty),
                    )
            })
        {
            let ty = self.instantiate_ty(self.properties[property].ty, &value.arguments);
            return Some((property, application, ty));
        }
        for parent in self.interfaces[value.template].parents.clone() {
            let parent_ty = self.interface_applications[parent].canonical_type;
            let parent_ty = self.instantiate_ty(parent_ty, &value.arguments);
            let hir::Type::Interface(parent) = self.types[parent_ty] else {
                unreachable!("interface parent substitutions stay interface applications")
            };
            if let Some(property) =
                self.find_accessible_interface_application_property(parent, name, receiver_ty, seen)
            {
                return Some(property);
            }
        }
        None
    }

    pub(crate) fn lower_property_read(
        &mut self,
        property: hir::PropertyId,
        owner_application: Option<hir::MethodOwnerApplication>,
        receiver: Option<hir::Expr>,
        ty: TypeId,
        span: ast::Span,
    ) -> Option<hir::Expr> {
        let declaration = self.properties[property].clone();
        let getter = declaration.capability.getter();
        let accessor = self.property_getters[getter].clone();
        if !self.access_domain_allows(
            &accessor.access.lookup.0,
            receiver.as_ref().map(|receiver| receiver.ty),
        ) {
            self.error(
                span,
                format!(
                    "getter of property `{}` is not accessible",
                    declaration.name
                ),
            );
            return None;
        }
        match accessor.implementation {
            hir::PropertyAccessorImplementation::Storage => {
                self.property_storage_read(&declaration, owner_application, receiver, ty, span)
            }
            hir::PropertyAccessorImplementation::Constant => {
                debug_assert!(owner_application.is_none());
                debug_assert!(receiver.is_none());
                let hir::PropertyRepresentation::Const { value } = declaration.representation
                else {
                    unreachable!("constant accessors belong only to const properties")
                };
                let kind = match value {
                    hir::ConstPropertyValue::Integer(value) => hir::ExprKind::IntLiteral(value),
                    hir::ConstPropertyValue::Boolean(value) => hir::ExprKind::BoolLiteral(value),
                    hir::ConstPropertyValue::String(value) => hir::ExprKind::StringLiteral(value),
                };
                Some(hir::Expr {
                    kind,
                    ty,
                    span,
                    origin: self.expression_origin(span),
                })
            }
            hir::PropertyAccessorImplementation::Body(function)
            | hir::PropertyAccessorImplementation::AbstractSlot(function) => {
                self.check_call_effects(hir::Callable::Function(function), span);
                match (owner_application, receiver) {
                    (Some(owner), Some(receiver)) => {
                        let callable =
                            hir::Callable::Method(self.record_method_application(function, owner));
                        Some(hir::Expr {
                            kind: hir::ExprKind::MethodCall {
                                receiver: Box::new(receiver),
                                callee: hir::MethodCallee::Callable(callable),
                                args: Vec::new(),
                            },
                            ty,
                            span,
                            origin: self.expression_origin(span),
                        })
                    }
                    (None, None) => Some(hir::Expr {
                        kind: hir::ExprKind::Call {
                            callee: hir::Callable::Function(function),
                            args: Vec::new(),
                        },
                        ty,
                        span,
                        origin: self.expression_origin(span),
                    }),
                    _ => unreachable!("property owner and receiver shapes are paired"),
                }
            }
        }
    }

    pub(crate) fn lower_property_write(
        &mut self,
        property: hir::PropertyId,
        owner_application: Option<hir::MethodOwnerApplication>,
        receiver: Option<hir::Expr>,
        value: hir::Expr,
        span: ast::Span,
    ) -> Option<hir::StatementKind> {
        let declaration = self.properties[property].clone();
        let Some(setter) = declaration.capability.setter() else {
            self.error(
                span,
                format!("cannot assign to immutable property `{}`", declaration.name),
            );
            return None;
        };
        let accessor = self.property_setters[setter].clone();
        if !self.access_domain_allows(
            &accessor.access.lookup.0,
            receiver.as_ref().map(|receiver| receiver.ty),
        ) {
            self.error(
                span,
                format!(
                    "setter of property `{}` is not accessible",
                    declaration.name
                ),
            );
            return None;
        }
        match accessor.implementation {
            hir::PropertyAccessorImplementation::Storage => {
                self.property_storage_write(&declaration, owner_application, receiver, value)
            }
            hir::PropertyAccessorImplementation::Constant => {
                unreachable!("const properties never expose a setter")
            }
            hir::PropertyAccessorImplementation::Body(function)
            | hir::PropertyAccessorImplementation::AbstractSlot(function) => {
                self.check_call_effects(hir::Callable::Function(function), span);
                let expression = match (owner_application, receiver) {
                    (Some(owner), Some(receiver)) => {
                        let callable =
                            hir::Callable::Method(self.record_method_application(function, owner));
                        hir::Expr {
                            kind: hir::ExprKind::MethodCall {
                                receiver: Box::new(receiver),
                                callee: hir::MethodCallee::Callable(callable),
                                args: vec![value],
                            },
                            ty: self.unit,
                            span,
                            origin: self.expression_origin(span),
                        }
                    }
                    (None, None) => hir::Expr {
                        kind: hir::ExprKind::Call {
                            callee: hir::Callable::Function(function),
                            args: vec![value],
                        },
                        ty: self.unit,
                        span,
                        origin: self.expression_origin(span),
                    },
                    _ => unreachable!("property owner and receiver shapes are paired"),
                };
                Some(hir::StatementKind::Expr(expression))
            }
        }
    }

    fn property_storage_read(
        &mut self,
        declaration: &hir::Property,
        owner_application: Option<hir::MethodOwnerApplication>,
        receiver: Option<hir::Expr>,
        ty: TypeId,
        span: ast::Span,
    ) -> Option<hir::Expr> {
        let kind = match &declaration.representation {
            hir::PropertyRepresentation::Stored(stored) => match stored.backing {
                hir::PropertyBacking::ClassField { field, .. } => {
                    let receiver = receiver.expect("class storage access has a receiver");
                    let Some(hir::MethodOwnerApplication::Class(application)) = owner_application
                    else {
                        unreachable!("class storage access has a class owner application")
                    };
                    hir::ExprKind::FieldAccess {
                        receiver: Box::new(receiver),
                        field: hir::FieldRef::ClassField { application, field },
                    }
                }
                hir::PropertyBacking::StructField { owner: _, index } => {
                    let receiver = receiver.expect("struct storage access has a receiver");
                    let Some(hir::MethodOwnerApplication::Struct(application)) = owner_application
                    else {
                        unreachable!("struct storage access has a struct owner application")
                    };
                    hir::ExprKind::FieldAccess {
                        receiver: Box::new(receiver),
                        field: hir::FieldRef::StructField { application, index },
                    }
                }
            },
            hir::PropertyRepresentation::NativeStorage { storage } => {
                debug_assert!(receiver.is_none());
                if matches!(
                    self.globals[*storage].storage,
                    hir::GlobalStorage::Extern { .. }
                ) {
                    self.require_unsafe_operation(span, "reading an extern global");
                }
                hir::ExprKind::GlobalRead(*storage)
            }
            hir::PropertyRepresentation::AccessorOnly
            | hir::PropertyRepresentation::Delegated { .. }
            | hir::PropertyRepresentation::Const { .. } => {
                unreachable!("only stored and native properties have storage accessors")
            }
        };
        Some(hir::Expr {
            kind,
            ty,
            span,
            origin: self.expression_origin(span),
        })
    }

    fn property_storage_write(
        &mut self,
        declaration: &hir::Property,
        owner_application: Option<hir::MethodOwnerApplication>,
        receiver: Option<hir::Expr>,
        value: hir::Expr,
    ) -> Option<hir::StatementKind> {
        let target = match &declaration.representation {
            hir::PropertyRepresentation::Stored(stored) => match stored.backing {
                hir::PropertyBacking::ClassField { field, .. } => {
                    let receiver = receiver.expect("class storage write has a receiver");
                    let Some(hir::MethodOwnerApplication::Class(application)) = owner_application
                    else {
                        unreachable!("class storage write has a class owner application")
                    };
                    hir::AssignTarget::Field {
                        receiver: Box::new(receiver),
                        field: hir::FieldRef::ClassField { application, field },
                    }
                }
                hir::PropertyBacking::StructField { .. } => {
                    unreachable!("value-type stored properties are immutable")
                }
            },
            hir::PropertyRepresentation::NativeStorage { storage } => {
                debug_assert!(receiver.is_none());
                if matches!(
                    self.globals[*storage].storage,
                    hir::GlobalStorage::Extern { .. }
                ) {
                    self.require_unsafe_operation(value.span, "writing an extern global");
                }
                hir::AssignTarget::Global(*storage)
            }
            hir::PropertyRepresentation::AccessorOnly
            | hir::PropertyRepresentation::Delegated { .. }
            | hir::PropertyRepresentation::Const { .. } => {
                unreachable!("only stored and native properties have storage setters")
            }
        };
        Some(hir::StatementKind::Assign { target, value })
    }

    pub(crate) fn contextual_backing_field(
        &mut self,
        span: ast::Span,
    ) -> Option<(hir::Expr, Option<hir::AssignTarget>)> {
        let context = self.backing_field_context.clone()?;
        if self.capture_contexts.len() > context.capture_depth {
            self.error(
                span,
                "`field` is only available in the direct property accessor body".to_string(),
            );
            return None;
        }
        let physical = self.class_fields[context.field].clone();
        let receiver = self.lower_current_this(span)?;
        let hir::Type::Class(application) = self.types[receiver.ty] else {
            unreachable!("a class backing field accessor has a class receiver")
        };
        let application_value = self.class_applications[application].clone();
        let ty = self.instantiate_ty(physical.ty, &application_value.arguments);
        let field = hir::FieldRef::ClassField {
            application,
            field: context.field,
        };
        let read = hir::Expr {
            kind: hir::ExprKind::FieldAccess {
                receiver: Box::new(receiver.clone()),
                field,
            },
            ty,
            span,
            origin: self.expression_origin(span),
        };
        let write = self.properties[physical.property]
            .capability
            .setter()
            .map(|_| hir::AssignTarget::Field {
                receiver: Box::new(receiver),
                field,
            });
        Some((read, write))
    }
}
