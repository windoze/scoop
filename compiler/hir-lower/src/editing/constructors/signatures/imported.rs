use super::*;
use crate::semantic_queries::imported_location;
use hir::ImportedCallableSource;

impl Lowerer {
    pub(in crate::editing::constructors) fn editing_imported_constructor_signature(
        &mut self,
        callee: C,
        identity: CallableTemplateOrigin,
        origin: hir::SemanticSourceLocation,
    ) -> Result<Option<hir::EditingConstructorSignature>, ast::Diagnostic> {
        let declaration = self
            .dependencies
            .as_ref()
            .expect("an imported constructor retains its catalog")
            .callable_declaration(identity)
            .expect("the selected constructor has its declaration");
        let interface = declaration.interface();
        let hir::PublicDeclarationOwnerV1::Nominal(owner) = interface.owner() else {
            unreachable!("constructors belong to their original nominal declaration")
        };
        let type_parameters = self
            .imported_callable_owner_parameters(owner)
            .map_err(|error| {
                ast::Diagnostic::at(origin.span, error.diagnostic("queried constructor owner"))
            })?;
        let bindings = self.query_signature_bindings(type_parameters.len(), &type_parameters);
        let substitutions = self.complete_parameter_substitution(type_parameters.iter(), &[]);
        let parameters = self.query_imported_parameters(&declaration, &bindings, &substitutions)?;
        let result = self
            .imported_signature_type_with_bindings(interface.result(), &bindings)
            .map_err(|error| {
                ast::Diagnostic::at(origin.span, error.diagnostic("queried constructor result"))
            })?;
        let form = match identity {
            CallableTemplateOrigin::VariantConstructor(variant) => {
                let owner = self
                    .dependencies
                    .as_ref()
                    .and_then(|dependencies| dependencies.nominal_declaration(owner))
                    .expect("the variant retains its enum");
                let hir::NominalSourceShapeV1::Enum(shape) = owner.interface.source_shape() else {
                    unreachable!("variant constructors belong to enums")
                };
                let variant = shape
                    .variants()
                    .iter()
                    .find(|entry| entry.variant() == variant)
                    .expect("the source enum retains its variant");
                let style = match variant.style() {
                    hir::EnumSourceVariantStyleV1::Unit => hir::VariantStyle::Unit,
                    hir::EnumSourceVariantStyleV1::Constructor => hir::VariantStyle::Constructor,
                    hir::EnumSourceVariantStyleV1::Positional => hir::VariantStyle::Positional,
                    hir::EnumSourceVariantStyleV1::Named => hir::VariantStyle::Named,
                };
                hir::EditingConstructorForm::Variant {
                    name: declaration.name().into(),
                    style,
                }
            }
            _ => hir::EditingConstructorForm::Nominal,
        };
        let mut selected = Vec::new();
        for parameter in parameters {
            let (vararg_element_type, default) = match parameter.calling {
                hir::SemanticParameterCalling::Required => (None, None),
                hir::SemanticParameterCalling::Default(default) => (None, Some(default)),
                hir::SemanticParameterCalling::Vararg {
                    element_type,
                    omission,
                } => (
                    Some(element_type.declaration),
                    match omission {
                        hir::SemanticVarargOmission::EmptyArray => None,
                        hir::SemanticVarargOmission::Default(default) => Some(default),
                    },
                ),
            };
            let declared_default = default
                .map(|default| {
                    let hir::SemanticDefaultReference::Imported(key) = default.definition else {
                        unreachable!("imported constructor parameters retain imported defaults")
                    };
                    let template = self
                        .dependencies
                        .as_ref()
                        .and_then(|dependencies| dependencies.default_template(key))
                        .expect("a checked default retains its template");
                    imported_location(template.definition_origin().origin())
                })
                .transpose()?;
            selected.push(hir::EditingParameter {
                name: parameter.name,
                value_type: parameter.value_type.declaration,
                vararg_element_type,
                declared_default,
                origin: parameter.origin,
            });
        }
        Ok(Some(hir::EditingConstructorSignature {
            callee,
            form,
            binders: hir::SemanticDeclarationBinders {
                owner: type_parameters,
                callable: Vec::new(),
            },
            parameters: selected,
            result,
            origin,
        }))
    }
}
