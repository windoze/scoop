use super::*;
use hir::ImportedCallableSource;

impl Lowerer {
    pub(super) fn query_imported_constructors(
        &mut self,
        result: hir::SemanticTypeUse,
        owner: &hir::ImportedNominalDeclaration,
        type_parameters: &[hir::TypeParamDecl],
        substitutions: &[(hir::TypeParamId, hir::TypeId)],
    ) -> Result<Vec<hir::SemanticConstructorDescription>, Diagnostic> {
        let details = owner.interface.declaration_details();
        let mut declarations = Vec::new();
        match self.types[result.applied] {
            hir::Type::Enum(application) => {
                for index in 0..self.enum_definition(owner.owner()).variants.len() {
                    let variant = self.enum_variant_at(application, index as u32);
                    declarations.push((
                        CallableTemplateOrigin::VariantConstructor(variant.variant),
                        hir::SemanticConstructorKind::Variant(variant.variant),
                    ));
                }
            }
            hir::Type::Struct(_) | hir::Type::Class(_)
                if owner.interface.kind() != hir::PublicNominalKindV1::Object =>
            {
                let primary = match owner.interface.kind() {
                    hir::PublicNominalKindV1::Struct => details.primary_value_constructor(),
                    hir::PublicNominalKindV1::Class => details
                        .class_primary_constructor()
                        .map(|primary| primary.constructor()),
                    _ => None,
                };
                for &constructor in details.constructors().values() {
                    let kind = if Some(constructor) == primary {
                        hir::SemanticConstructorKind::Primary
                    } else {
                        hir::SemanticConstructorKind::Secondary
                    };
                    declarations.push((CallableTemplateOrigin::Constructor(constructor), kind));
                }
            }
            _ => return Ok(Vec::new()),
        }
        let bindings = self.query_signature_bindings(type_parameters.len(), type_parameters);
        let mut constructors = Vec::new();
        for (identity, kind) in declarations {
            let declaration = self
                .dependencies
                .as_ref()
                .expect("an imported constructor retains its catalog")
                .callable_declaration(identity)
                .expect("a checked nominal retains its source constructor declaration");
            let mut parameters =
                self.query_imported_parameters(&declaration, &bindings, substitutions)?;
            for (index, parameter) in parameters.iter_mut().enumerate() {
                parameter.target = match kind {
                    hir::SemanticConstructorKind::Primary => match self.types[result.applied] {
                        hir::Type::Struct(_) => {
                            hir::StaticParameterTarget::Field(hir::StaticFieldIdentity::Field(
                                owner.interface.source_shape().declared_fields()[index].field(),
                            ))
                        }
                        hir::Type::Class(_) => details
                            .class_primary_constructor()
                            .expect("a primary class constructor retains its parameter relation")
                            .properties()[index]
                            .map_or(
                                hir::StaticParameterTarget::Plain,
                                hir::StaticParameterTarget::Property,
                            ),
                        _ => unreachable!("primary constructors belong to structs or classes"),
                    },
                    hir::SemanticConstructorKind::Variant(variant) => {
                        let hir::Type::Enum(application) = self.types[result.applied] else {
                            unreachable!("a variant constructor belongs to its enum")
                        };
                        let variant = hir::EnumVariantApplication {
                            owner: application,
                            variant,
                        };
                        hir::StaticParameterTarget::Field(hir::StaticFieldIdentity::VariantField(
                            self.enum_variant_field_at(variant, index as u32).field,
                        ))
                    }
                    hir::SemanticConstructorKind::Secondary => hir::StaticParameterTarget::Plain,
                };
            }
            let interface = declaration.interface();
            constructors.push(hir::SemanticConstructorDescription {
                identity,
                kind,
                parameters,
                result,
                access: hir::SemanticMemberAccess {
                    visibility: interface.declared_visibility(),
                    domain: self
                        .imported_member_access_domain(owner, interface.declared_visibility()),
                    accessible: self.imported_callable_is_accessible(interface, None),
                },
                annotations: hir::SemanticAnnotationTarget::Imported(
                    hir::AnnotationTargetV1::Callable(hir::AnnotationCallableV1::Declaration(
                        identity,
                    )),
                ),
                origin: imported_location(declaration.definition_origin().origin())?,
            });
        }
        constructors.sort_by_key(|constructor| {
            (
                constructor.kind != hir::SemanticConstructorKind::Primary,
                constructor.origin.span.start,
            )
        });
        Ok(constructors)
    }
}
