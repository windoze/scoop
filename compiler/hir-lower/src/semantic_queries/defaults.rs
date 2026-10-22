use super::*;
use hir::{ImportedCallableSource, SemanticQueries};

impl FrontendQueries<'_, '_> {
    pub(super) fn query_default_application(
        &mut self,
        context: hir::SemanticTypeContext<'_>,
        application: &hir::SemanticDefaultUse,
    ) -> Result<Option<hir::SemanticDefaultApplication>, Diagnostic> {
        let Some(definition) = self.body_queries.default(application.definition)? else {
            return Ok(None);
        };
        let provider = self.default_provider(application.definition)?;
        let arguments = application
            .arguments
            .iter()
            .map(|&ty| self.type_signature(context, ty))
            .collect::<Result<Vec<_>, _>>()?;
        let arguments = hir::CanonicalBinderUseListV1::try_new(arguments).map_err(error)?;
        hir::SemanticDefaultApplication::try_new(definition, provider, arguments)
            .map(Some)
            .map_err(error)
    }

    fn default_provider(
        &self,
        reference: hir::SemanticDefaultReference,
    ) -> Result<hir::DefaultTemplateProviderShapeV1, Diagnostic> {
        let definition = self
            .default_definition(reference)
            .expect("a projected default retains its bound definition");
        let (owner, own) = match definition {
            hir::SemanticDefaultDefinition::Current { template, .. } => {
                match template.definition_root {
                    hir::LexicalDefinitionRoot::Function(id) => {
                        let function = &self.lowerer.functions[id];
                        let owner = function.owner_type_param_count();
                        (owner, function.type_param_count() - owner)
                    }
                    hir::LexicalDefinitionRoot::StructConstructor(id) => {
                        let owner = self.lowerer.struct_constructors[id].owner;
                        (self.lowerer.structs[owner].type_params.len(), 0)
                    }
                    hir::LexicalDefinitionRoot::ClassConstructor(id) => {
                        let owner = self.lowerer.class_constructors[id].owner;
                        (self.lowerer.classes[owner].type_params.len(), 0)
                    }
                    hir::LexicalDefinitionRoot::VariantConstructor(id) => {
                        (self.lowerer.enums[id.enumeration()].type_params.len(), 0)
                    }
                }
            }
            hir::SemanticDefaultDefinition::Imported(template) => {
                let dependencies = self
                    .lowerer
                    .dependencies
                    .as_ref()
                    .expect("an imported default retains its catalog");
                let declaration = dependencies
                    .callable_declaration(template.definition_root().declaration())
                    .map_err(error)?;
                let interface = declaration.interface();
                let owner = match interface.owner().nominal_owner() {
                    Some(owner) => dependencies
                        .nominal_declaration(owner)
                        .expect("the bound provider retains its nominal declaration")
                        .interface
                        .type_parameters()
                        .binders()
                        .len(),
                    None => 0,
                };
                (owner, interface.type_parameters().binders().len())
            }
        };
        hir::DefaultTemplateProviderShapeV1::try_new(
            u32::try_from(owner).map_err(error)?,
            u32::try_from(own).map_err(error)?,
        )
        .map_err(error)
    }
}

fn error(cause: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::at(
        Span::new(0, 0),
        format!("cannot project the queried default application: {cause}"),
    )
}
