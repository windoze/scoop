use super::*;
use hir::ImportedCallableSource;

impl Lowerer {
    fn find_initializing_class_field(
        &mut self,
        application: hir::ClassApplicationId,
        name: &str,
    ) -> Option<(
        hir::InitializingClassFieldRef,
        TypeId,
        bool,
        hir::SemanticProperty,
    )> {
        let application_value = self.class_applications[application].clone();
        let receiver = self.initializing_receiver_type()?;
        let Some(class) = self.source_class_id(application_value.template) else {
            return self.find_imported_initializing_field(
                application_value.canonical_type,
                receiver,
                name,
            );
        };
        if let Some(&property_id) = self.classes[class]
            .properties
            .iter()
            .find(|property| self.properties[**property].name == name)
        {
            self.infer_property_type(property_id)?;
            let property = self.properties[property_id].clone();
            if !self.property_is_accessible(property_id, Some(receiver)) {
                return None;
            }
            let hir::PropertyRepresentation::Stored(stored) = property.representation else {
                return None;
            };
            let hir::PropertyBacking::ClassField { field, .. } = stored.backing else {
                return None;
            };
            let ty = self.instantiate_ty(property.ty, &application_value.arguments);
            let mutable = property.capability.setter().is_some_and(|setter| {
                self.property_accessor_is_accessible(
                    property_id,
                    &self.property_setters[setter].access,
                    Some(receiver),
                )
            });
            return Some((
                self.initializing_class_field_reference(application, field),
                ty,
                mutable,
                hir::SemanticProperty::Current(property_id),
            ));
        }
        let base = self.classes[class].base_class?;
        let base = self.instantiate_ty(base, &application_value.arguments);
        match self.types[base] {
            Type::Class(application) => self.find_initializing_class_field(application, name),
            _ => unreachable!("resolved class bases have class types"),
        }
    }

    fn find_imported_initializing_field(
        &mut self,
        base: TypeId,
        receiver: TypeId,
        name: &str,
    ) -> Option<(
        hir::InitializingClassFieldRef,
        TypeId,
        bool,
        hir::SemanticProperty,
    )> {
        use scoop_identity::{CallableTemplateOrigin, PropertyOwner};
        let candidates = self
            .imported_member_candidates_for_receiver(
                base,
                Some(receiver),
                hir::ImportedMemberLookup::PropertyGetter(name),
            )
            .expect("initialized dependency bases have complete member declarations");
        let [getter] = candidates.as_slice() else {
            return None;
        };
        let CallableTemplateOrigin::Accessor(accessor) = getter.interface().declaration() else {
            unreachable!("property lookup returns an accessor")
        };
        let dependencies = self.dependencies.as_ref()?;
        let property = dependencies.property_for_accessor(accessor)?;
        let property_target = hir::SemanticProperty::Imported(property.declaration());
        if property.representation() != hir::PropertyRepresentationV1::RuntimeAccessor {
            return None;
        }
        let PropertyOwner::Property(property_id) = property.declaration() else {
            unreachable!("nominal fields belong to ordinary properties")
        };
        let hir::PublicDeclarationOwnerV1::Nominal(owner) = property.owner() else {
            unreachable!("a dependency class field has a nominal owner")
        };
        let declaration = dependencies.nominal_declaration(owner)?;
        let field = declaration
            .field_sources
            .iter()
            .zip(declaration.interface.source_shape().declared_fields())
            .find_map(|(source, field)| {
                (source.storage.backing_property() == Some(property_id)).then_some(field.field())
            })?;
        let mutable = property.accessors().setter().is_some_and(|setter| {
            let setter = dependencies
                .callable_declaration(CallableTemplateOrigin::Accessor(setter))
                .expect("a property retains its setter declaration");
            self.imported_callable_is_accessible(setter.interface(), Some(receiver))
        });
        let owner = self
            .imported_member_owner_type(base, owner)
            .expect("an initialized base retains its complete declaring application");
        let Type::Class(application) = self.types[owner] else {
            unreachable!("a dependency backing field belongs to a class")
        };
        let application = self.class_applications[application].clone();
        let class = &self.loaded_class_definitions[&application.template];
        let ty = class.definition.fields[class.field_index(field)?].ty;
        let ty = self.instantiate_ty(ty, &application.arguments);
        Some((
            hir::InitializingClassFieldRef { owner, field },
            ty,
            mutable,
            property_target,
        ))
    }

    pub(crate) fn initializing_receiver_type(&self) -> Option<TypeId> {
        match &self.initialization_context.as_ref()?.receiver {
            InitializingReceiver::Class { application, .. } => {
                Some(self.class_applications[*application].canonical_type)
            }
            InitializingReceiver::Struct { application } => {
                Some(self.struct_applications[*application].canonical_type)
            }
        }
    }

    pub(crate) fn initializing_field(
        &mut self,
        name: &ast::Ident,
        span: ast::Span,
        access: hir::EditingPropertyAccess,
    ) -> Option<InitializingField> {
        let context = self.initialization_context.clone()?;
        match context.receiver {
            InitializingReceiver::Class { application, .. } => {
                let class = self.class_applications[application].template;
                let Some((field, ty, mutable, property)) =
                    self.find_initializing_class_field(application, &name.text)
                else {
                    self.error(
                        name.span,
                        format!(
                            "class `{}` has no field `{}`",
                            self.classes[self.class_id(class)].name,
                            name.text
                        ),
                    );
                    return None;
                };
                if let Some(declaration) =
                    self.field_identity_builder.class_declaration(field.field)
                {
                    self.require_initialized_class_field(declaration, &name.text, name.span)?;
                }
                self.observe_property_reference(span, property, ty, access);
                let read = hir::Expr {
                    kind: hir::ExprKind::InitializingClassFieldAccess { field },
                    ty,
                    span,
                    origin: self.expression_origin(span),
                };
                let write = mutable.then_some(hir::AssignTarget::InitializingClassField {
                    field,
                    origin: self.expression_origin(span),
                });
                Some(InitializingField { read, write })
            }
            InitializingReceiver::Struct { application } => {
                let application_value = self.struct_applications[application].clone();
                let structure = application_value.template;
                let fields = self.struct_definition(structure).semantic_fields();
                let Some(index) = fields.iter().position(|field| field.name == name.text) else {
                    self.error(
                        name.span,
                        format!(
                            "struct `{}` has no field `{}`",
                            self.nominal_template_name(structure),
                            name.text
                        ),
                    );
                    return None;
                };
                let ty = self.instantiate_ty(fields[index].ty, &application_value.arguments);
                if self.observes_current_body() {
                    let structure = self.struct_id(structure);
                    let property = self.structs[structure].properties.iter().copied().find(|property| matches!(
                        &self.properties[*property].representation,
                        hir::PropertyRepresentation::Stored(hir::StoredProperty {
                            backing: hir::PropertyBacking::StructField { owner, index: field }, ..
                        }) if *owner == structure && *field as usize == index
                    )).expect("a declared struct field has its original logical property");
                    self.observe_property_reference(
                        span,
                        hir::SemanticProperty::Current(property),
                        ty,
                        access,
                    );
                }
                let hir::FieldRef::StructField { owner, field } =
                    self.struct_field_reference(application, index as u32)
                else {
                    unreachable!("a struct projection selects a struct field")
                };
                Some(InitializingField {
                    read: hir::Expr {
                        kind: hir::ExprKind::InitializingStructFieldAccess { owner, field },
                        ty,
                        span,
                        origin: self.expression_origin(span),
                    },
                    write: None,
                })
            }
        }
    }

    pub(crate) fn initializing_receiver_has_field(&mut self, name: &str) -> bool {
        let Some(context) = self.initialization_context.clone() else {
            return false;
        };
        match context.receiver {
            InitializingReceiver::Class { application, .. } => self
                .find_initializing_class_field(application, name)
                .is_some(),
            InitializingReceiver::Struct { application } => {
                let structure = self.struct_applications[application].template;
                self.struct_definition(structure)
                    .semantic_fields()
                    .iter()
                    .any(|field| field.name == name)
            }
        }
    }

    pub(crate) fn is_initializing_receiver(&self, receiver: Option<&hir::Expr>) -> bool {
        self.initialization_context.is_some()
            && matches!(
                receiver.map(|receiver| &receiver.kind),
                Some(hir::ExprKind::Local(local))
                    if self.current_this.is_some_and(|(current, _)| current == *local)
            )
    }

    pub(crate) fn check_initializing_named_property(&mut self, name: &ast::Ident) -> Option<()> {
        if let Some(receiver) = self.initializing_receiver_type()
            && let Some((property, _, _)) =
                self.find_accessible_nominal_property(receiver, &name.text)
        {
            self.check_initializing_property(property, name.span)?;
        }
        Some(())
    }

    pub(crate) fn check_initializing_property(
        &mut self,
        property: hir::PropertyId,
        span: ast::Span,
    ) -> Option<()> {
        let declaration = &self.properties[property];
        let field = match &declaration.representation {
            hir::PropertyRepresentation::Stored(hir::StoredProperty {
                backing: hir::PropertyBacking::ClassField { field, .. },
            }) => Some(*field),
            hir::PropertyRepresentation::Delegated { storage } => {
                match self.delegate_storages[*storage].location {
                    hir::DelegateStorageLocation::ClassField(field) => Some(field),
                    hir::DelegateStorageLocation::ManagedGlobal(_) => None,
                }
            }
            hir::PropertyRepresentation::AccessorOnly
                if matches!(
                    declaration.owner,
                    hir::PropertyOwner::Class(_) | hir::PropertyOwner::Object(_)
                ) && self.property_accessor_sources.iter().any(|source| {
                    source.property == property
                        && source.body_kind == crate::properties::PropertyAccessorBodyKind::Delegate
                }) =>
            {
                // The delegate field is allocated only after its initializer
                // establishes the effective storage type.
                let name = declaration.name.clone();
                return self.uninitialized_field_error(&name, span);
            }
            _ => None,
        };
        if let Some(field) = field {
            let name = declaration.name.clone();
            self.require_initialized_class_field(field, &name, span)?;
        }
        Some(())
    }

    fn require_initialized_class_field(
        &mut self,
        field: hir::ClassFieldId,
        name: &str,
        span: ast::Span,
    ) -> Option<()> {
        if let Some(crate::InitializationContext {
            receiver: InitializingReceiver::Class { initialized, .. },
            ..
        }) = &self.initialization_context
            && !initialized.contains(&field)
        {
            return self.uninitialized_field_error(name, span);
        }
        Some(())
    }

    fn uninitialized_field_error(&mut self, name: &str, span: ast::Span) -> Option<()> {
        let step = &self
            .initialization_context
            .as_ref()
            .expect("field readiness is checked in an initialization context")
            .step;
        self.error(
            span,
            format!(
                "field `{name}` is not initialized during {step}; initializing receiver cannot observe a field before its store completes"
            ),
        );
        None
    }
}
