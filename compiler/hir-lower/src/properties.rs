use scoop_ast as ast;
use scoop_hir as hir;

use crate::{Lowerer, Owner, TypeId};

mod receiver;
pub(crate) use receiver::PropertyCallReceiver;

mod access;
mod declarations;
mod delegates;
mod extensions;
mod implicit_values;
mod lookup;
mod method_callees;
mod storage;

pub(crate) use delegates::{DelegateRoleCall, ResolvedDelegateRoleCall};
pub(crate) use extensions::{
    ExtensionPropertyCandidateOutcome, ExtensionPropertyResolution, ResolvedExtensionPropertyRead,
    ResolvedExtensionPropertyWrite,
};
pub(crate) use implicit_values::ImplicitValueResolution;

#[derive(Clone)]
pub(crate) struct PropertyAccessorSource {
    pub(crate) property: hir::PropertyId,
    pub(crate) kind: PropertyAccessorKind,
    pub(crate) function: hir::FunctionId,
    pub(crate) declaration: ast::FunctionDecl,
    pub(crate) owner: Option<Owner>,
    pub(crate) backing: Option<hir::PropertyBacking>,
    pub(crate) body_kind: PropertyAccessorBodyKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PropertyAccessorBodyKind {
    Source,
    Storage,
    Delegate,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PropertyAccessorKind {
    Getter,
    Setter,
}

#[derive(Clone)]
pub(crate) struct LocalDelegatePlan {
    pub(crate) property_ty: TypeId,
    pub(crate) mutable: bool,
    pub(crate) getter: LocalDelegateAccessor,
    pub(crate) setter: Option<LocalDelegateAccessor>,
}

#[derive(Clone)]
pub(crate) struct LocalDelegateAccessor {
    pub(crate) dispatch: LocalDelegateDispatch,
    pub(crate) effect: DelegateCallEffect,
    pub(crate) receiver_ty: TypeId,
    pub(crate) parameter_types: Vec<TypeId>,
    pub(crate) result_ty: TypeId,
}

#[derive(Clone, Copy)]
pub(crate) enum DelegateCallEffect {
    Current(hir::Callable),
    Imported(hir::CallableSafetyV1),
}

#[derive(Clone)]
pub(crate) enum LocalDelegateDispatch {
    Member(hir::MethodCallee),
    Call {
        callee: hir::CallableTarget,
        binding: Option<std::sync::Arc<hir::DirectImportedTargetBinding>>,
        receiver: hir::SourceCallReceiver<TypeId>,
    },
}

#[derive(Clone)]
pub(crate) struct BackingFieldContext {
    pub(crate) backing: hir::PropertyBacking,
    pub(crate) capture_depth: usize,
}

impl Lowerer {
    pub(crate) fn qualified_object_const_property(
        &mut self,
        receiver: &ast::Expr,
        name: &str,
    ) -> Result<Option<hir::PropertyId>, ()> {
        let Some((target, _)) = self.complete_companion_qualifier(receiver)? else {
            return Ok(None);
        };
        let direct = match target {
            crate::NominalTarget::Object(object) => self.object_const_property(object, name),
            _ => None,
        };
        Ok(direct.or_else(|| {
            let companion = self.companion_object(target.owner())?;
            self.object_const_property(companion, name)
        }))
    }

    fn object_const_property(&self, object: hir::ObjectId, name: &str) -> Option<hir::PropertyId> {
        self.classes[self.objects[object].backing_class]
            .properties
            .iter()
            .copied()
            .find(|property| {
                self.properties[*property].name == name
                    && matches!(
                        self.properties[*property].representation,
                        hir::PropertyRepresentation::Const { .. }
                    )
                    && self.access_domain_allows(&self.properties[*property].access.lookup.0)
            })
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
        self.record_property_initialization_dependency(&declaration, span);
        let getter = declaration.capability.getter();
        let accessor = self.property_getters[getter].clone();
        if !self.property_accessor_is_accessible(
            property,
            &accessor.access,
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
        // A local raw access retains its storage identity; the generated
        // accessor body supplies the ordinary cross-Cone callable.
        if matches!(
            declaration.representation,
            hir::PropertyRepresentation::NativeStorage { .. }
        ) {
            return self.property_storage_read(&declaration, owner_application, receiver, ty, span);
        }
        match accessor.implementation {
            hir::PropertyAccessorImplementation::Storage => {
                self.property_storage_read(&declaration, owner_application, receiver, ty, span)
            }
            hir::PropertyAccessorImplementation::Constant => {
                let hir::PropertyRepresentation::Const { value } = declaration.representation
                else {
                    unreachable!("constant accessors belong only to const properties")
                };
                let kind = match value {
                    hir::ConstPropertyValue::Integer(value) => hir::ExprKind::IntegerLiteral(value),
                    hir::ConstPropertyValue::Float(value) => hir::ExprKind::FloatLiteral(value),
                    hir::ConstPropertyValue::Char(value) => hir::ExprKind::CharLiteral(value),
                    hir::ConstPropertyValue::Boolean(value) => hir::ExprKind::BoolLiteral(value),
                    hir::ConstPropertyValue::String(value) => hir::ExprKind::StringLiteral {
                        value,
                        owner: hir::StringConstantOwner::Property(property),
                    },
                };
                Some(hir::Expr {
                    kind,
                    ty,
                    span,
                    origin: self.expression_origin(span),
                })
            }
            hir::PropertyAccessorImplementation::Body(function)
            | hir::PropertyAccessorImplementation::StorageBody(function)
            | hir::PropertyAccessorImplementation::AbstractSlot(function) => {
                self.check_call_effects(hir::Callable::Function(function), span);
                match (owner_application, receiver) {
                    (Some(owner), Some(receiver)) => {
                        let callee = self.property_method_callee(function, owner, receiver.ty);
                        Some(hir::Expr {
                            kind: hir::ExprKind::MethodCall {
                                receiver: Box::new(receiver),
                                callee,
                                args: Vec::new(),
                            },
                            ty,
                            span,
                            origin: self.expression_origin(span),
                        })
                    }
                    (None, None) => Some(hir::Expr {
                        kind: hir::ExprKind::Call {
                            binding: None,
                            receiver: hir::SourceCallReceiver::NoReceiver,
                            callee: (hir::Callable::Function(function)).into(),
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
        self.record_property_initialization_dependency(&declaration, span);
        let Some(setter) = declaration.capability.setter() else {
            self.error(
                span,
                format!("cannot assign to immutable property `{}`", declaration.name),
            );
            return None;
        };
        let accessor = self.property_setters[setter].clone();
        if !self.property_accessor_is_accessible(
            property,
            &accessor.access,
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
        if matches!(
            declaration.representation,
            hir::PropertyRepresentation::NativeStorage { .. }
        ) {
            return self.property_storage_write(&declaration, owner_application, receiver, value);
        }
        match accessor.implementation {
            hir::PropertyAccessorImplementation::Storage => {
                self.property_storage_write(&declaration, owner_application, receiver, value)
            }
            hir::PropertyAccessorImplementation::Constant => {
                unreachable!("const properties never expose a setter")
            }
            hir::PropertyAccessorImplementation::Body(function)
            | hir::PropertyAccessorImplementation::StorageBody(function)
            | hir::PropertyAccessorImplementation::AbstractSlot(function) => {
                self.check_call_effects(hir::Callable::Function(function), span);
                let expression = match (owner_application, receiver) {
                    (Some(owner), Some(receiver)) => {
                        let callee = self.property_method_callee(function, owner, receiver.ty);
                        hir::Expr {
                            kind: hir::ExprKind::MethodCall {
                                receiver: Box::new(receiver),
                                callee,
                                args: vec![value],
                            },
                            ty: self.unit,
                            span,
                            origin: self.expression_origin(span),
                        }
                    }
                    (None, None) => hir::Expr {
                        kind: hir::ExprKind::Call {
                            binding: None,
                            receiver: hir::SourceCallReceiver::NoReceiver,
                            callee: (hir::Callable::Function(function)).into(),
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

    fn record_property_initialization_dependency(
        &mut self,
        declaration: &hir::Property,
        span: ast::Span,
    ) {
        let Some(current) = self.current_initialization_unit else {
            return;
        };
        let dependency = match declaration.representation {
            hir::PropertyRepresentation::Stored(hir::StoredProperty {
                backing:
                    hir::PropertyBacking::TopLevelGlobal {
                        initialization: hir::TopLevelInitialization::Runtime(unit),
                        ..
                    },
            }) => Some(unit),
            hir::PropertyRepresentation::Delegated { storage } => {
                match self.delegate_storages[storage].location {
                    hir::DelegateStorageLocation::ManagedGlobal(global) => {
                        match self.globals[global].storage {
                            hir::GlobalStorage::Managed {
                                state: hir::HirStaticInitialState::ZeroedForRuntimeUnit { unit },
                            } => Some(unit),
                            _ => {
                                unreachable!("a global delegate storage has runtime initialization")
                            }
                        }
                    }
                    hir::DelegateStorageLocation::ClassField(_) => None,
                }
            }
            hir::PropertyRepresentation::Stored(_)
            | hir::PropertyRepresentation::AccessorOnly
            | hir::PropertyRepresentation::GenericDelegated { .. }
            | hir::PropertyRepresentation::Const { .. }
            | hir::PropertyRepresentation::NativeStorage { .. } => None,
        };
        let Some(dependency) = dependency else {
            return;
        };
        let dependencies = &mut self.initialization_units[current].dependencies;
        if !dependencies
            .iter()
            .any(|existing| existing.unit == dependency)
        {
            dependencies.push(hir::InitializationDependency {
                unit: dependency,
                type_arguments: Vec::new(),
                span,
            });
        }
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
        match context.backing {
            hir::PropertyBacking::TopLevelGlobal { storage, .. } => {
                let global = self.globals[storage].clone();
                let read = hir::Expr {
                    kind: hir::ExprKind::GlobalRead(storage),
                    ty: global.ty,
                    span,
                    origin: self.expression_origin(span),
                };
                let write = self.properties[global.property]
                    .capability
                    .setter()
                    .map(|_| hir::AssignTarget::Global(storage));
                Some((read, write))
            }
            hir::PropertyBacking::ClassField { field, .. } => {
                let physical = self.class_fields[field].clone();
                let receiver = self.lower_current_this(span)?;
                let hir::Type::Class(application) = self.types[receiver.ty] else {
                    unreachable!("a class backing field accessor has a class receiver")
                };
                let application_value = self.class_applications[application].clone();
                let ty = self.instantiate_ty(
                    self.class_field_definition(field).ty,
                    &application_value.arguments,
                );
                let field_ref = self.class_field_reference(application, field);
                let read = hir::Expr {
                    kind: hir::ExprKind::FieldAccess {
                        receiver: Box::new(receiver.clone()),
                        field: field_ref,
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
                        field: field_ref,
                    });
                Some((read, write))
            }
            hir::PropertyBacking::StructField { .. } => {
                unreachable!("struct properties do not expose contextual backing fields")
            }
        }
    }
}
