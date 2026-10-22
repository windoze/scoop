use super::*;

impl Lowerer {
    pub(super) fn prepare_inline_templates(&mut self) -> Result<(), String> {
        let declarations = self
            .imported_dependency_callables
            .iter()
            .filter_map(|(_, use_)| {
                let declaration = use_.reference().declaration();
                if !matches!(declaration, CallableTemplateOrigin::Function(_))
                    || self
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
                (declaration.interface().owner() == hir::PublicDeclarationOwnerV1::TopLevel
                    && declaration.callable_body().is_some()
                    && declaration
                        .interface()
                        .type_parameters()
                        .binders()
                        .is_empty())
                .then_some(declaration)
            })
            .collect::<Vec<_>>();
        for declaration in declarations {
            let key = declaration.interface().declaration();
            let CallableTemplateOrigin::Function(owner) = key else {
                unreachable!()
            };
            let prepared = self.prepare_imported_generic(
                declaration,
                hir::ImportedCallableTemplateOrigin::Inline(owner),
            )?;
            self.insert_imported_template(key, prepared);
        }
        Ok(())
    }
}
