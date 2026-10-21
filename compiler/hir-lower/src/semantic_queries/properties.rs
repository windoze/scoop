//! Logical properties use the same access and substitution rules as expressions.

use super::*;

mod accessors;
mod imported;

impl Lowerer {
    pub(super) fn query_nominal_properties(
        &mut self,
        ty: hir::TypeId,
    ) -> Result<Option<Vec<hir::SemanticPropertyDescription>>, Diagnostic> {
        let Some(application) = self.nominal_application(ty) else {
            return Ok(None);
        };
        let parameters = self
            .nominal_declaration_parameters(application.template)
            .expect("a resolved nominal retains complete declaration parameters");
        let bindings = parameters
            .iter()
            .zip(&application.arguments)
            .map(|(parameter, argument)| (parameter.id, *argument))
            .collect::<Vec<_>>();
        if let Some((dependency, _)) = self.dependency_nominal_application(ty) {
            let dependency = dependency.clone();
            return self
                .query_imported_properties(ty, &dependency, &parameters, &bindings)
                .map(Some);
        }
        let owner = self.nominal_owners[&application.template];
        let properties = match owner {
            Owner::Struct(id) => &self.structs[id].properties,
            Owner::Enum(id) => &self.enums[id].properties,
            Owner::Class(id) => &self.classes[id].properties,
            Owner::Interface(id) => &self.interfaces[id].properties,
            Owner::Object(id) => &self.classes[self.objects[id].backing_class].properties,
        }
        .clone();
        Ok(Some(
            properties
                .into_iter()
                .map(|id| self.query_current_property(ty, id, &bindings))
                .collect(),
        ))
    }

    fn query_current_property(
        &mut self,
        receiver: hir::TypeId,
        property: hir::PropertyId,
        bindings: &[(hir::TypeParamId, hir::TypeId)],
    ) -> hir::SemanticPropertyDescription {
        let declaration = self.properties[property].clone();
        let identity = self
            .ordinary_property_identity(property)
            .ordinary_id()
            .expect("nominal properties have ordinary identities");
        let file = self
            .property_definition_file(property)
            .expect("source properties retain their definition file");
        let getter =
            self.query_current_getter(receiver, property, declaration.capability.getter(), file);
        let setter = declaration
            .capability
            .setter()
            .map(|setter| self.query_current_setter(receiver, property, setter, file));
        hir::SemanticPropertyDescription {
            identity,
            name: declaration.name,
            value_type: self.query_type_use(declaration.ty, bindings),
            visibility: declaration.access.declared.into(),
            lookup_domain: declaration.access.lookup.0,
            getter,
            setter,
            overrides: declaration
                .overrides
                .iter()
                .map(|reference| match reference {
                    hir::PropertyReference::Local(id) => self
                        .ordinary_property_identity(*id)
                        .ordinary_id()
                        .expect("an overridden nominal property has an ordinary identity"),
                    hir::PropertyReference::Imported { declaration, .. } => *declaration,
                })
                .collect(),
            annotations: hir::SemanticAnnotationTarget::Current(
                hir::SourceAnnotationTarget::Property(property),
            ),
            origin: hir::SemanticSourceLocation {
                source: self.source_identity(file).clone(),
                span: declaration.span,
            },
        }
    }
}
