use super::*;

impl FrontendQueries<'_, '_> {
    pub(super) fn default_provider(
        &mut self,
        reference: hir::SemanticDefaultReference,
    ) -> Result<(hir::DefaultTemplateProviderShapeV1, Vec<hir::TypeParamDecl>), Diagnostic> {
        let definition = self
            .default_definition(reference)
            .expect("a projected default retains its bound definition");
        let (owner, parameters) = match definition {
            hir::SemanticDefaultDefinition::Current { template, .. } => {
                match template.definition_root {
                    hir::LexicalDefinitionRoot::Function(id) => {
                        let signature = &self.lowerer.signatures[&id];
                        (
                            signature.owner_type_param_count,
                            signature.type_params.clone(),
                        )
                    }
                    hir::LexicalDefinitionRoot::StructConstructor(id) => {
                        let owner = self.lowerer.struct_constructors[id].owner;
                        let parameters = self.lowerer.structs[owner].type_params.clone();
                        (parameters.len(), parameters)
                    }
                    hir::LexicalDefinitionRoot::ClassConstructor(id) => {
                        let owner = self.lowerer.class_constructors[id].owner;
                        let parameters = self.lowerer.classes[owner].type_params.clone();
                        (parameters.len(), parameters)
                    }
                    hir::LexicalDefinitionRoot::VariantConstructor(id) => {
                        let parameters = self.lowerer.enums[id.enumeration()].type_params.clone();
                        (parameters.len(), parameters)
                    }
                }
            }
            hir::SemanticDefaultDefinition::Imported(template) => {
                let identity = template.definition_root().declaration();
                let declaration = self
                    .lowerer
                    .dependencies
                    .as_ref()
                    .expect("an imported default retains its catalog")
                    .callable_declaration(identity)
                    .map_err(error)?;
                let parameters = match declaration.interface().owner().nominal_owner() {
                    Some(owner) => self
                        .lowerer
                        .nominal_declaration_parameters(owner)
                        .map_err(|cause| error(cause.diagnostic("default provider")))?,
                    None => Vec::new(),
                };
                let owner = parameters.len();
                let parameters = self.lowerer.query_imported_method_binders(
                    &declaration,
                    &parameters,
                    Span::new(0, 0),
                )?;
                (owner, parameters)
            }
        };
        let shape = hir::DefaultTemplateProviderShapeV1::try_new(
            u32::try_from(owner).map_err(error)?,
            u32::try_from(parameters.len() - owner).map_err(error)?,
        )
        .map_err(error)?;
        Ok((shape, parameters))
    }
}
