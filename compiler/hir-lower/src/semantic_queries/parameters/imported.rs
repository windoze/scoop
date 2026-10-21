use super::*;
use hir::ImportedCallableSource;

impl Lowerer {
    pub(in crate::semantic_queries) fn query_imported_parameters(
        &mut self,
        declaration: &hir::ImportedCallableDeclaration,
        bindings: &ImportedTypeBindings,
        substitutions: &[(hir::TypeParamId, hir::TypeId)],
    ) -> Result<Vec<hir::SemanticParameterDescription>, Diagnostic> {
        let identity = declaration.interface().declaration();
        let source = declaration
            .source_interface()
            .expect("a source callable retains its full parameter interface");
        source
            .parameters()
            .parameters()
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                let calling = match parameter.calling() {
                    hir::CallableParameterCallingV1::Required => {
                        hir::SemanticParameterCalling::Required
                    }
                    hir::CallableParameterCallingV1::Default { template } => {
                        hir::SemanticParameterCalling::Default(self.query_imported_default(
                            *template,
                            bindings,
                            substitutions,
                        )?)
                    }
                    hir::CallableParameterCallingV1::VarargEmpty { element_type } => {
                        hir::SemanticParameterCalling::Vararg {
                            element_type: self.query_published_type_use(
                                element_type,
                                bindings,
                                substitutions,
                            )?,
                            omission: hir::SemanticVarargOmission::EmptyArray,
                        }
                    }
                    hir::CallableParameterCallingV1::VarargDefault {
                        element_type,
                        template,
                    } => hir::SemanticParameterCalling::Vararg {
                        element_type: self.query_published_type_use(
                            element_type,
                            bindings,
                            substitutions,
                        )?,
                        omission: hir::SemanticVarargOmission::Default(
                            self.query_imported_default(*template, bindings, substitutions)?,
                        ),
                    },
                };
                let index = u32::try_from(index).expect("source parameter arity fits u32");
                Ok(hir::SemanticParameterDescription {
                    identity: hir::SemanticParameterIdentity {
                        owner: identity,
                        index,
                    },
                    name: parameter.name().as_str().to_owned(),
                    value_type: self.query_published_type_use(
                        parameter.value_type(),
                        bindings,
                        substitutions,
                    )?,
                    calling,
                    target: hir::StaticParameterTarget::Plain,
                    annotations: hir::SemanticAnnotationTarget::Imported(
                        hir::AnnotationTargetV1::Parameter {
                            callable: hir::AnnotationCallableV1::Declaration(identity),
                            index,
                        },
                    ),
                    origin: imported_location(parameter.definition_origin().origin())?,
                })
            })
            .collect()
    }

    pub(in crate::semantic_queries) fn query_published_type_use(
        &mut self,
        signature: &SignatureTypeKey,
        bindings: &ImportedTypeBindings,
        substitutions: &[(hir::TypeParamId, hir::TypeId)],
    ) -> Result<hir::SemanticTypeUse, Diagnostic> {
        let declaration = self
            .imported_signature_type_with_bindings(signature, bindings)
            .map_err(|_| {
                Diagnostic::at(
                    Span::new(0, 0),
                    "cannot resolve the queried dependency signature type",
                )
            })?;
        Ok(self.query_type_use(declaration, substitutions))
    }

    fn query_imported_default(
        &mut self,
        key: hir::ExportDefaultTemplateKeyV1,
        bindings: &ImportedTypeBindings,
        substitutions: &[(hir::TypeParamId, hir::TypeId)],
    ) -> Result<hir::SemanticDefaultUse, Diagnostic> {
        let arguments = self
            .dependencies
            .as_ref()
            .expect("an imported default retains its catalog")
            .default_template(key)
            .expect("checked default parameters retain their selected template")
            .type_parameters()
            .arguments()
            .to_vec();
        let arguments = arguments
            .iter()
            .map(|ty| {
                self.query_published_type_use(ty, bindings, substitutions)
                    .map(|ty| ty.applied)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(hir::SemanticDefaultUse {
            definition: hir::SemanticDefaultReference::Imported(key),
            arguments,
        })
    }
}
