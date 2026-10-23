use super::*;

impl BodyQueries<'_> {
    pub(in crate::semantic_queries) fn default(
        &mut self,
        reference: hir::SemanticDefaultReference,
    ) -> Result<Option<Arc<hir::ExportDefaultTemplateV1>>, Diagnostic> {
        match reference {
            hir::SemanticDefaultReference::Current(source) => {
                self.current_default(source).map(Some)
            }
            hir::SemanticDefaultReference::Imported(key) => {
                if let Some(template) = self.imported_defaults.get(&key) {
                    return Ok(Some(Arc::clone(template)));
                }
                let Some(template) = self
                    .lowerer
                    .dependencies
                    .as_ref()
                    .and_then(|dependencies| dependencies.default_template(key))
                else {
                    return Ok(None);
                };
                let template = Arc::new(template.clone());
                self.imported_defaults.insert(key, Arc::clone(&template));
                Ok(Some(template))
            }
        }
    }

    fn current_default(
        &mut self,
        source: hir::ExportDefaultSourceId,
    ) -> Result<Arc<hir::ExportDefaultTemplateV1>, Diagnostic> {
        if let Some(template) = self.defaults.get(&source) {
            return Ok(Arc::clone(template));
        }
        let (owner, position) = self
            .lowerer
            .source_parameter_interfaces
            .iter()
            .find_map(|interface| {
                interface
                    .parameters
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| parameter_default(&parameter.calling) == Some(source))
                    .map(|(position, _)| (interface.owner, position as u32))
            })
            .ok_or_else(|| projection_error("default has no bound parameter interface"))?;
        let data = self.data()?;
        let template = data
            .inputs(self.lowerer)
            .project_parameter_default(owner, position, source, Some(&data.imported))
            .map_err(projection_error)?;
        let template = Arc::new(template);
        self.defaults.insert(source, Arc::clone(&template));
        Ok(template)
    }
}

fn parameter_default(calling: &hir::ExportParameterCalling) -> Option<hir::ExportDefaultSourceId> {
    match *calling {
        hir::ExportParameterCalling::Default { source, .. }
        | hir::ExportParameterCalling::Vararg {
            omission: hir::ExportVarargOmission::Default(source),
            ..
        } => Some(source),
        hir::ExportParameterCalling::Required { .. }
        | hir::ExportParameterCalling::Vararg {
            omission: hir::ExportVarargOmission::EmptyArray,
            ..
        } => None,
    }
}
