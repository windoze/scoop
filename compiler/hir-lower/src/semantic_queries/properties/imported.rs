use super::*;
use hir::ImportedCallableSource;
use scoop_identity::{CallableTemplateOrigin, PropertyOwner};

impl Lowerer {
    pub(super) fn query_imported_properties(
        &mut self,
        receiver: hir::TypeId,
        owner: &hir::ImportedNominalDeclaration,
        parameters: &[hir::TypeParamDecl],
        substitutions: &[(hir::TypeParamId, hir::TypeId)],
    ) -> Result<Vec<hir::SemanticPropertyDescription>, Diagnostic> {
        let bindings = parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                (
                    SignatureTypeKey::Binder {
                        depth: 0,
                        index: u32::try_from(index).expect("declaration arity fits u32"),
                    },
                    self.intern_type(hir::Type::Param(parameter.id)),
                )
            })
            .collect::<crate::imported_core::ImportedTypeBindings>();
        let mut properties = Vec::new();
        for member in owner.interface.declaration_details().members().values() {
            let hir::NestedSourceMemberRefV1::Property(identity) = *member else {
                continue;
            };
            let dependencies = self
                .dependencies
                .as_ref()
                .expect("an imported owner retains its catalog");
            let property = PropertyOwner::Property(identity);
            let declaration = dependencies
                .property_declaration(property)
                .expect("the checked nominal retains its actual property declarations")
                .clone();
            let name = dependencies
                .property_name(property)
                .expect("a property retains its source name")
                .to_owned();
            let origin = imported_location(
                dependencies
                    .property_definition_origin(property)
                    .expect("a property retains its definition origin"),
            )?;
            let declaration_type = self
                .imported_signature_type_with_bindings(declaration.value_type(), &bindings)
                .map_err(|_| {
                    Diagnostic::at(
                        Span::new(0, 0),
                        "cannot resolve the queried dependency property type",
                    )
                })?;
            let getter = self.query_imported_accessor(
                receiver,
                owner,
                declaration.accessors().getter_source(),
            )?;
            let setter = declaration
                .accessors()
                .setter_source()
                .map(|source| self.query_imported_accessor(receiver, owner, source))
                .transpose()?;
            properties.push(hir::SemanticPropertyDescription {
                identity,
                name,
                value_type: self.query_type_use(declaration_type, substitutions),
                visibility: declaration.declared_visibility(),
                lookup_domain: self
                    .imported_member_access_domain(owner, declaration.declared_visibility()),
                getter,
                setter,
                overrides: declaration.overrides().to_vec(),
                annotations: hir::SemanticAnnotationTarget::Imported(
                    hir::AnnotationTargetV1::Property(identity),
                ),
                origin,
            });
        }
        properties.sort_by_key(|property| (property.origin.span.start, property.identity));
        Ok(properties)
    }

    fn query_imported_accessor(
        &self,
        receiver: hir::TypeId,
        owner: &hir::ImportedNominalDeclaration,
        source: hir::PropertyAccessorSourceV1,
    ) -> Result<hir::SemanticAccessorDescription, Diagnostic> {
        let identity = source.accessor();
        let callable = CallableTemplateOrigin::Accessor(identity);
        let declaration = self
            .dependencies
            .as_ref()
            .expect("an imported accessor retains its catalog")
            .callable_declaration(callable)
            .expect("a property's capability retains its actual accessor declaration");
        let interface = declaration.interface();
        Ok(hir::SemanticAccessorDescription {
            identity,
            callable: hir::SemanticCallable::Imported(
                hir::DefaultCallableDeclarationV1::PropertyAccessor(identity),
            ),
            access: hir::SemanticMemberAccess {
                visibility: interface.declared_visibility(),
                domain: self.imported_member_access_domain(owner, interface.declared_visibility()),
                accessible: self.imported_callable_is_accessible(interface, Some(receiver)),
            },
            implementation: source.implementation(),
            annotations: hir::SemanticAnnotationTarget::Imported(
                hir::AnnotationTargetV1::Callable(hir::AnnotationCallableV1::Declaration(callable)),
            ),
            origin: imported_location(declaration.definition_origin().origin())?,
        })
    }
}
