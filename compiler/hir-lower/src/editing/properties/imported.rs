use super::*;
use scoop_identity::CallableTemplateOrigin;

impl Lowerer {
    pub(super) fn editing_imported_property_declaration(
        &mut self,
        property: hir::PropertyDeclarationId,
    ) -> Option<hir::EditingPropertyDeclaration> {
        let dependencies = self.dependencies.as_ref()?;
        let declaration = dependencies.property_declaration(property)?.clone();
        let name = dependencies.property_name(property)?.to_owned();
        let getter = dependencies
            .callable_declaration(CallableTemplateOrigin::Accessor(
                declaration.accessors().getter(),
            ))
            .ok()?;
        let origin = self.editing_property_origin(Property::Imported(property));
        let span = origin
            .as_ref()
            .map_or(ast::Span::new(0, 0), |origin| origin.span);
        let owner = match declaration.owner() {
            hir::PublicDeclarationOwnerV1::Nominal(owner) => {
                self.imported_callable_owner_parameters(owner).ok()?
            }
            hir::PublicDeclarationOwnerV1::TopLevel | hir::PublicDeclarationOwnerV1::Extension => {
                Vec::new()
            }
        };
        let parameters = self
            .query_imported_method_binders(&getter, &owner, span)
            .ok()?;
        let bindings = self.query_signature_bindings(owner.len(), &parameters);
        let value_type = self
            .imported_signature_type_with_bindings(declaration.value_type(), &bindings)
            .ok()?;
        let receiver = declaration
            .receiver()
            .map(|receiver| self.imported_signature_type_with_bindings(receiver, &bindings))
            .transpose()
            .ok()?;
        let binders = hir::SemanticDeclarationBinders {
            callable: parameters[owner.len()..].to_vec(),
            owner,
        };
        Some(hir::EditingPropertyDeclaration {
            property: Property::Imported(property),
            name,
            value_type,
            receiver,
            binders,
            origin,
            mutable: declaration.accessors().setter().is_some(),
        })
    }
}
