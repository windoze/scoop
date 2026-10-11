use super::*;
use crate::globals::PendingRuntimeInitializerKind;

impl Lowerer {
    pub(super) fn infer_property_initializer(
        &mut self,
        property: hir::PropertyId,
        source: &ast::PropertyDecl,
        expression: &ast::Expr,
    ) -> Option<TypeId> {
        let owner = match self.properties[property].owner {
            hir::PropertyOwner::Class(owner) => Some(owner),
            hir::PropertyOwner::Object(object) => Some(self.objects[object].backing_class),
            _ => None,
        };
        if let Some(owner) = owner {
            return self.infer_class_property_initializer(owner, property, source, expression);
        }
        if matches!(
            self.properties[property].representation,
            hir::PropertyRepresentation::NativeStorage { .. }
        ) {
            let accessor = self.property_accessor_source(property, PropertyAccessorKind::Getter);
            let mut declaration = accessor.declaration;
            declaration.body = ast::FunctionBody::Expr(Box::new(expression.clone()));
            self.lower_body(accessor.function, &declaration);
            return (!self.signatures[&accessor.function].result_pending)
                .then_some(self.signatures[&accessor.function].return_ty);
        }
        let pending = self
            .pending_runtime_initializers
            .iter()
            .find(|pending| match pending.kind {
                PendingRuntimeInitializerKind::Stored { storage, .. } => {
                    self.globals[storage].property == property
                }
                PendingRuntimeInitializerKind::Delegated {
                    property: candidate,
                    ..
                } => candidate == property,
            })
            .cloned()
            .expect("an inferred top-level initializer has an eager unit");
        self.lower_runtime_initializer(&pending)
    }

    fn infer_class_property_initializer(
        &mut self,
        owner: hir::ClassId,
        property: hir::PropertyId,
        source: &ast::PropertyDecl,
        expression: &ast::Expr,
    ) -> Option<TypeId> {
        let constructors = self.classes[owner].constructors.clone();
        let primary = constructors.iter().copied().find(|constructor| {
            matches!(
                self.class_constructors[*constructor].kind,
                hir::ClassConstructorKind::Primary { .. }
            )
        });
        let constructor = primary.or_else(|| constructors.first().copied())?;
        let primary_parameters_visible =
            primary.is_some() && !self.object_by_backing_class.contains_key(&owner);
        let mut initialized = self.inherited_fields(owner);
        let before = self.classes[owner]
            .properties
            .iter()
            .take_while(|candidate| **candidate != property)
            .copied()
            .collect::<std::collections::HashSet<_>>();
        for field in self.classes[owner].fields.clone() {
            if matches!(
                self.class_fields[field].source,
                hir::ClassFieldSource::PrimaryParameter { .. }
                    | hir::ClassFieldSource::InterfaceDelegate { .. }
                    | hir::ClassFieldSource::EnclosingInstance
                    | hir::ClassFieldSource::CapturedEnvironment
            ) || self.class_fields[field]
                .source
                .property()
                .is_some_and(|property| before.contains(&property))
            {
                initialized.insert(field);
            }
        }
        self.initialization_context = Some(crate::InitializationContext {
            receiver: crate::InitializingReceiver::Class {
                application: self.classes[owner].self_application,
                initialized,
            },
            step: format!(
                "initializer of property `{}` in class `{}`",
                source.name.text, self.classes[owner].name
            ),
            capture_depth: self.capture_contexts.len(),
        });
        if let Some(&object) = self.object_by_backing_class.get(&owner) {
            self.current_initialization_unit =
                Some(self.singleton_values[self.objects[object].singleton_value].initialization);
        }
        let step = if matches!(source.body, ast::PropertyBodySyntax::Delegated { .. }) {
            self.lower_class_delegate_initialization(
                owner,
                constructor,
                primary_parameters_visible,
                property,
                source,
                expression,
            )?
        } else {
            let lowered = self.with_property_initializer_context(
                constructor,
                property,
                "stored property initializer",
                |this, sink| {
                    if !primary_parameters_visible {
                        this.constructor_params_in_scope.clear();
                    }
                    this.lower_expr(expression, sink, None)
                },
            )?;
            let hir::PropertyRepresentation::Stored(ref stored) =
                self.properties[property].representation
            else {
                unreachable!("an instance initializer owns a field")
            };
            let hir::PropertyBacking::ClassField { field, .. } = stored.backing else {
                unreachable!("an instance initializer owns a class field")
            };
            hir::ClassInitializationStep::Field {
                field: self.initializing_class_field_reference(
                    self.classes[owner].self_application,
                    field,
                ),
                initializer: hir::ConstructorExpression {
                    locals: lowered.locals,
                    statements: lowered.statements,
                    value: lowered.value,
                },
                span: source.span,
            }
        };
        let hir::ClassInitializationStep::Field {
            ref initializer, ..
        } = step
        else {
            unreachable!("a property initializer writes its backing field")
        };
        let ty = initializer.value.ty;
        self.declaration_inference
            .field_initializers
            .insert(property, step);
        self.initialization_context = None;
        self.current_initialization_unit = None;
        Some(ty)
    }
}
