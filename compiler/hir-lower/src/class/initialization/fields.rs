use super::*;
use hir::ImportedCallableSource;

impl Lowerer {
    fn find_initializing_class_field(
        &mut self,
        application: hir::ClassApplicationId,
        name: &str,
    ) -> Option<(hir::InitializingClassFieldRef, TypeId, bool)> {
        let application_value = self.class_applications[application].clone();
        let class = application_value.template;
        let receiver = self.initializing_receiver_type()?;
        if let Some(&property_id) = self.classes[class]
            .properties
            .iter()
            .find(|property| self.properties[**property].name == name)
        {
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
                hir::InitializingClassFieldRef::Declared { application, field },
                ty,
                mutable,
            ));
        }
        let base = self.classes[class].base_class?;
        let base = self.instantiate_ty(base, &application_value.arguments);
        match self.types[base] {
            Type::Class(application) => self.find_initializing_class_field(application, name),
            Type::ImportedClass(_) => self.find_imported_initializing_field(base, receiver, name),
            _ => unreachable!("resolved class bases have class types"),
        }
    }

    fn find_imported_initializing_field(
        &mut self,
        base: TypeId,
        receiver: TypeId,
        name: &str,
    ) -> Option<(hir::InitializingClassFieldRef, TypeId, bool)> {
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
        if property.representation() != hir::PropertyRepresentationV1::RuntimeAccessor {
            return None;
        }
        let PropertyOwner::Property(property_id) = property.declaration() else {
            unreachable!("nominal fields belong to ordinary properties")
        };
        let hir::PublicDeclarationOwnerV1::Nominal(hir::SourceNominalId::Concrete(owner)) =
            property.owner()
        else {
            unreachable!("a dependency class field has a concrete owner")
        };
        let declaration = dependencies.nominal(owner)?;
        let field = declaration
            .field_sources
            .iter()
            .zip(declaration.interface.source_shape().declared_fields())
            .find_map(|(source, field)| {
                (source.backing_property == Some(property_id)).then_some(field.field())
            })?;
        let mutable = property.accessors().setter().is_some_and(|setter| {
            let setter = dependencies
                .callable_declaration(CallableTemplateOrigin::Accessor(setter))
                .expect("a property retains its setter declaration");
            self.imported_callable_is_accessible(setter.interface(), Some(receiver))
        });
        let owner = self
            .imported_signature_type(&scoop_identity::SignatureTypeKey::Nominal(owner))
            .expect("an initialized base retains its concrete declaring type");
        let Type::ImportedClass(class) = &self.types[owner] else {
            unreachable!("a dependency backing field belongs to a class")
        };
        let ty = class
            .fields
            .iter()
            .find(|source| source.identity == field)?
            .ty;
        Some((
            hir::InitializingClassFieldRef::Imported { owner, field },
            ty,
            mutable,
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
    ) -> Option<InitializingField> {
        let context = self.initialization_context.clone()?;
        if self.capture_contexts.len() > context.capture_depth {
            self.error(
                span,
                "initializing receiver cannot escape before construction completes".into(),
            );
            return None;
        }
        match context.receiver {
            InitializingReceiver::Class {
                application,
                initialized,
            } => {
                let class = self.class_applications[application].template;
                let Some((field, ty, mutable)) =
                    self.find_initializing_class_field(application, &name.text)
                else {
                    self.error(
                        name.span,
                        format!(
                            "class `{}` has no field `{}`",
                            self.classes[class].name, name.text
                        ),
                    );
                    return None;
                };
                if let hir::InitializingClassFieldRef::Declared { field, .. } = field
                    && !initialized.contains(&field)
                {
                    self.error(
                        name.span,
                        format!(
                            "field `{}` is not initialized during {}; initializing receiver cannot observe a field before its store completes",
                            name.text, context.step
                        ),
                    );
                    return None;
                }
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
                let fields = self.structs[structure].semantic_fields();
                let Some(index) = fields.iter().position(|field| field.name == name.text) else {
                    self.error(
                        name.span,
                        format!(
                            "struct `{}` has no field `{}`",
                            self.structs[structure].name, name.text
                        ),
                    );
                    return None;
                };
                let ty = self.instantiate_ty(fields[index].ty, &application_value.arguments);
                Some(InitializingField {
                    read: hir::Expr {
                        kind: hir::ExprKind::InitializingStructFieldAccess {
                            application,
                            index: index as u32,
                        },
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
                self.structs[structure]
                    .semantic_fields()
                    .iter()
                    .any(|field| field.name == name)
            }
        }
    }

    pub(crate) fn reject_initializing_this(&mut self, span: ast::Span) -> bool {
        if self.initialization_context.is_none() {
            return false;
        }
        self.error(
            span,
            "initializing receiver cannot escape before construction completes".into(),
        );
        true
    }
}
