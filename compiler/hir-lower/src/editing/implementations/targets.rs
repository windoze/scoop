use super::*;

impl DeclarationAnalysis {
    pub fn type_implementation_targets(
        &mut self,
        source: &scoop_identity::SourceIdentity,
        span: ast::Span,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<V<Vec<hir::SourceNominalId>>, EditingCancelled> {
        let check = std::rc::Rc::new(is_cancelled);
        let cancellation = check.clone();
        match self.type_reference(source, span, move || cancellation())? {
            V::Known(Some(reference)) => match reference.target {
                Identity::Nominal(declaration) => return Ok(V::Known(vec![declaration])),
                Identity::Alias(_) => {
                    return self
                        .source_type_application(source, span, move || check())
                        .map(|value| match value {
                            V::Known(ty) => V::Known(
                                ty.and_then(|ty| self.lowerer.semantic_nominal_identity(ty))
                                    .into_iter()
                                    .collect(),
                            ),
                            V::Unavailable(reason) => V::Unavailable(reason),
                        });
                }
                Identity::Parameter(_) | Identity::Annotation(_) => {
                    return Ok(V::Known(Vec::new()));
                }
            },
            V::Unavailable(reason) => return Ok(V::Unavailable(reason)),
            V::Known(None) => {}
        }
        let bindings = self
            .import_bindings(source, span)
            .cloned()
            .collect::<Vec<_>>();
        let mut targets = Vec::new();
        for binding in bindings {
            if check() {
                return Err(EditingCancelled);
            }
            if let Some(target) = self.lowerer.editing_import_type_target(&binding)
                && let Some(Identity::Nominal(declaration)) =
                    self.lowerer.editing_type_identity(target)
                && !targets.contains(&declaration)
            {
                targets.push(declaration);
            }
        }
        Ok(V::Known(targets))
    }
}
