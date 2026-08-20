use super::*;

impl Lowerer {
    pub(crate) fn imported_context_parameters(
        &mut self,
        parameters: &[hir::SourceParameterShapeV1],
        bindings: &ImportedTypeBindings,
        span: scoop_ast::Span,
    ) -> Result<Vec<hir::ContextParameter>, String> {
        parameters
            .iter()
            .map(|parameter| {
                let name = parameter.name().as_str();
                Ok(hir::ContextParameter {
                    label: if name == "_" {
                        hir::ContextParameterLabel::Unnamed
                    } else {
                        hir::ContextParameterLabel::Named(name.to_owned())
                    },
                    ty: self.imported_generic_type(parameter.value_type(), bindings)?,
                    span,
                })
            })
            .collect()
    }
}
