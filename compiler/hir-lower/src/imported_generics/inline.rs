use super::*;

impl Lowerer {
    pub(super) fn prepare_inline_templates(&mut self, first_use: usize) -> Result<(), String> {
        let declarations = self
            .imported_dependency_callables
            .iter()
            .skip(first_use)
            .filter_map(|(_, use_)| {
                let declaration = use_.reference().declaration();
                if !matches!(
                    declaration,
                    CallableTemplateOrigin::Function(_) | CallableTemplateOrigin::Accessor(_)
                ) || self
                    .imported_generic_templates
                    .by_declaration
                    .contains_key(&declaration)
                {
                    return None;
                }
                let declaration = self
                    .dependencies
                    .as_ref()?
                    .callable_declaration(declaration)
                    .ok()?;
                declaration
                    .callable_body()
                    .is_some_and(|body| body.type_parameters().arguments().is_empty())
                    .then_some((declaration.interface().declaration(), declaration))
            })
            .collect::<BTreeMap<_, _>>();
        for (key, declaration) in declarations {
            let origin = match key {
                CallableTemplateOrigin::Function(owner) => {
                    hir::ImportedCallableTemplateOrigin::Inline(owner)
                }
                CallableTemplateOrigin::Accessor(owner) => {
                    hir::ImportedCallableTemplateOrigin::InlineAccessor(owner)
                }
                _ => unreachable!("ordinary inline declarations were selected above"),
            };
            let prepared = self.prepare_imported_generic(declaration, origin)?;
            self.insert_imported_template(key, prepared);
        }
        Ok(())
    }
}
