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
    pub(crate) backing: Option<hir::PropertyBacking>,
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
    pub(crate) backing: hir::PropertyBacking,
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

pub(crate) enum ExtensionPropertyCandidateOutcome {
    NoCandidate,
    NoApplicable,
    Failed,
    Resolved(Box<ResolvedExtensionProperty>),
}

pub(crate) enum ImplicitValueResolution {
    NoCandidate,
    NoApplicable(Box<Lowerer>),
    Failed,
    Value {
        target: crate::imports::lookup::values::ValueTarget,
        layer: crate::imports::ImportLookupLayer,
    },
    ExtensionProperty(Box<ResolvedExtensionProperty>),
}

impl Lowerer {
    pub(crate) fn qualified_object_const_property(
        &self,
        receiver: &ast::Expr,
        name: &str,
    ) -> Option<hir::PropertyId> {
        let target = self.nominal_qualifier_target(receiver)?;
        let direct = match target {
            crate::NominalTarget::Object(object) => self.object_const_property(object, name),
            _ => None,
        };
        direct.or_else(|| {
            let companion = self.companion_object(target.owner())?;
            self.object_const_property(companion, name)
        })
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
                    && self.access_domain_allows(&self.properties[*property].access.lookup.0, None)
            })
    }

    pub(crate) fn resolve_extension_property(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        sink: &mut Vec<hir::Statement>,
        require_read: bool,
    ) -> ExtensionPropertyResolution {
        let declared = self
            .top_level_namespaces
            .extension_property_layers(self.current_file, &name.text)
            .iter()
            .any(|properties| !properties.is_empty());
        let mut first_failure = None;
        for layer in self.named_extension_property_layers(&name.text) {
            let properties = layer
                .candidates
                .into_iter()
                .filter(|property| {
                    self.access_domain_allows(
                        &self.properties[*property].access.lookup.0,
                        Some(receiver.ty),
                    )
                })
                .collect::<Vec<_>>();
            if properties.is_empty() {
                continue;
            }
            let mut state = self.clone();
            let mut layer_sink = Vec::new();
            match state.resolve_extension_property_candidates_outcome(
                receiver.clone(),
                name,
                &properties,
                &mut layer_sink,
                require_read,
            ) {
                ExtensionPropertyCandidateOutcome::Resolved(resolved) => {
                    *self = state;
                    sink.extend(layer_sink);
                    return ExtensionPropertyResolution::Resolved(resolved);
                }
                ExtensionPropertyCandidateOutcome::NoApplicable => {
                    first_failure.get_or_insert(Box::new(state));
                }
                ExtensionPropertyCandidateOutcome::Failed => {
                    self.commit_layer_diagnostics(state);
                    return ExtensionPropertyResolution::Failed;
                }
                ExtensionPropertyCandidateOutcome::NoCandidate => {}
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

    /// Resolve one bare-name layer at a time after lexical and real-member
    /// lookup. Ordinary values and an extension property applicable to the
    /// implicit receiver are non-overloadable peers within the same layer.
    pub(crate) fn resolve_implicit_value(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        sink: &mut Vec<hir::Statement>,
        require_read: bool,
    ) -> ImplicitValueResolution {
        use crate::imports::lookup::calls::NamedCallTarget;

        let mut first_failure = None;
        for layer in self.named_call_layers(&name.text) {
            let mut values = Vec::new();
            let mut properties = Vec::new();
            let mut blockers = Vec::new();
            for binding in layer.candidates {
                let origin = self.named_call_value_origin(&binding);
                match binding.target {
                    NamedCallTarget::Value(value) => values.push((value, origin)),
                    NamedCallTarget::ExtensionProperty(property)
                        if self.access_domain_allows(
                            &self.properties[property].access.lookup.0,
                            Some(receiver.ty),
                        ) =>
                    {
                        properties.push((property, origin));
                    }
                    NamedCallTarget::Function(_)
                    | NamedCallTarget::ImportedCoreCallable(_)
                    | NamedCallTarget::ImportedDependency(_)
                    | NamedCallTarget::Type(_) => {
                        blockers.push(origin);
                    }
                    NamedCallTarget::ExtensionProperty(_) => {}
                }
            }

            let extension = if properties.is_empty() {
                ExtensionPropertyCandidateOutcome::NoCandidate
            } else {
                let property_ids = properties
                    .iter()
                    .map(|(property, _)| *property)
                    .collect::<Vec<_>>();
                let mut state = self.clone();
                let mut layer_sink = Vec::new();
                match state.resolve_extension_property_candidates_outcome(
                    receiver.clone(),
                    name,
                    &property_ids,
                    &mut layer_sink,
                    require_read,
                ) {
                    ExtensionPropertyCandidateOutcome::Resolved(property) => {
                        if !values.is_empty() {
                            let mut origins =
                                values.iter().map(|(_, origin)| *origin).collect::<Vec<_>>();
                            origins.push(
                                properties
                                    .iter()
                                    .find_map(|(candidate, origin)| {
                                        (*candidate == property.property).then_some(*origin)
                                    })
                                    .expect("a resolved extension property came from this layer"),
                            );
                            self.diagnose_value_layer(name, layer.kind, &origins);
                            return ImplicitValueResolution::Failed;
                        }
                        *self = state;
                        sink.extend(layer_sink);
                        return ImplicitValueResolution::ExtensionProperty(property);
                    }
                    ExtensionPropertyCandidateOutcome::NoApplicable => {
                        first_failure.get_or_insert(Box::new(state));
                        ExtensionPropertyCandidateOutcome::NoApplicable
                    }
                    ExtensionPropertyCandidateOutcome::Failed => {
                        self.commit_layer_diagnostics(state);
                        return ImplicitValueResolution::Failed;
                    }
                    ExtensionPropertyCandidateOutcome::NoCandidate => {
                        ExtensionPropertyCandidateOutcome::NoCandidate
                    }
                }
            };

            match values.as_slice() {
                [] => {}
                [(target, _)] => {
                    return ImplicitValueResolution::Value {
                        target: *target,
                        layer: layer.kind,
                    };
                }
                _ => {
                    let origins = values.iter().map(|(_, origin)| *origin).collect::<Vec<_>>();
                    self.diagnose_value_layer(name, layer.kind, &origins);
                    return ImplicitValueResolution::Failed;
                }
            }
            match extension {
                ExtensionPropertyCandidateOutcome::NoCandidate
                | ExtensionPropertyCandidateOutcome::NoApplicable => {}
                ExtensionPropertyCandidateOutcome::Failed
                | ExtensionPropertyCandidateOutcome::Resolved(_) => {
                    unreachable!("terminal extension outcomes return from their layer")
                }
            }
            if !blockers.is_empty() {
                self.diagnose_value_layer(name, layer.kind, &blockers);
                return ImplicitValueResolution::Failed;
            }
            if !layer.suppressed_callables.is_empty() {
                return ImplicitValueResolution::Failed;
            }
        }
        first_failure.map_or(
            ImplicitValueResolution::NoCandidate,
            ImplicitValueResolution::NoApplicable,
        )
    }

    pub(crate) fn resolve_extension_property_candidates_outcome(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        properties: &[hir::PropertyId],
        sink: &mut Vec<hir::Statement>,
        require_read: bool,
    ) -> ExtensionPropertyCandidateOutcome {
        if properties.is_empty() {
            return ExtensionPropertyCandidateOutcome::NoCandidate;
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
        let resolved = match self.resolve_extension_overload_outcome(
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
        ) {
            crate::overload::OverloadResolutionOutcome::NoApplicable => {
                return ExtensionPropertyCandidateOutcome::NoApplicable;
            }
            crate::overload::OverloadResolutionOutcome::Blocked => {
                return ExtensionPropertyCandidateOutcome::Failed;
            }
            crate::overload::OverloadResolutionOutcome::Failed => {
                return ExtensionPropertyCandidateOutcome::Failed;
            }
            crate::overload::OverloadResolutionOutcome::Resolved(resolved) => *resolved,
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
            let declaration = self.properties[property].clone();
            self.record_property_initialization_dependency(&declaration, name.span);
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
        ExtensionPropertyCandidateOutcome::Resolved(Box::new(ResolvedExtensionProperty {
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
        self.record_property_initialization_dependency(&property, span);
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
                let owner = match self.properties[property].owner {
                    hir::PropertyOwner::Object(object) => {
                        hir::MethodOwnerApplication::Object(self.objects[object].object_type)
                    }
                    _ => hir::MethodOwnerApplication::Class(declaring),
                };
                Some((property, owner, ty))
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
        self.record_property_initialization_dependency(&declaration, span);
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
                let hir::PropertyRepresentation::Const { value } = declaration.representation
                else {
                    unreachable!("constant accessors belong only to const properties")
                };
                let kind = match value {
                    hir::ConstPropertyValue::Integer(value) => hir::ExprKind::IntegerLiteral(value),
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
        self.record_property_initialization_dependency(&declaration, span);
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
                span,
            });
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
                hir::PropertyBacking::TopLevelGlobal { storage, .. } => {
                    debug_assert!(receiver.is_none());
                    hir::ExprKind::GlobalRead(storage)
                }
                hir::PropertyBacking::ClassField { field, .. } => {
                    let receiver = receiver.expect("class storage access has a receiver");
                    let application = match owner_application {
                        Some(hir::MethodOwnerApplication::Class(application)) => application,
                        Some(hir::MethodOwnerApplication::Object(object)) => {
                            self.object_types[object].representation
                        }
                        _ => {
                            unreachable!(
                                "reference storage access has a reference owner application"
                            )
                        }
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
                    let field = hir::AppliedStructFieldRef::checked(
                        &self.structs,
                        &self.struct_applications,
                        application,
                        index,
                    )
                    .expect("a struct-backed property names its declaring field");
                    hir::ExprKind::FieldAccess {
                        receiver: Box::new(receiver),
                        field: hir::FieldRef::StructField(field),
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
                hir::PropertyBacking::TopLevelGlobal { storage, .. } => {
                    debug_assert!(receiver.is_none());
                    hir::AssignTarget::Global(storage)
                }
                hir::PropertyBacking::ClassField { field, .. } => {
                    let receiver = receiver.expect("class storage write has a receiver");
                    let application = match owner_application {
                        Some(hir::MethodOwnerApplication::Class(application)) => application,
                        Some(hir::MethodOwnerApplication::Object(object)) => {
                            self.object_types[object].representation
                        }
                        _ => {
                            unreachable!(
                                "reference storage write has a reference owner application"
                            )
                        }
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
                let ty = self.instantiate_ty(physical.ty, &application_value.arguments);
                let field_ref = hir::FieldRef::ClassField { application, field };
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
