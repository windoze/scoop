use super::*;

impl Lowerer {
    pub(super) fn query_imported_method_binders(
        &mut self,
        declaration: &hir::ImportedCallableDeclaration,
        owner_parameters: &[hir::TypeParamDecl],
        span: Span,
    ) -> Result<Vec<hir::TypeParamDecl>, Diagnostic> {
        let identity = declaration.interface().declaration();
        if let Some(parameters) = self.query_imported_method_parameters.get(&identity) {
            return Ok(parameters.clone());
        }
        let binders = declaration.interface().type_parameters().binders();
        let shape = hir::DefaultTemplateProviderShapeV1::try_new(
            owner_parameters.len() as u32,
            binders.len() as u32,
        )
        .expect("checked signature binder counts fit the format");
        let mut bindings = ImportedTypeBindings::new();
        for (index, parameter) in owner_parameters.iter().enumerate() {
            bindings.insert(
                shape
                    .identity_binder_at(index as u32)
                    .expect("owner binder"),
                self.intern_type(hir::Type::Param(parameter.id)),
            );
        }
        let ids = binders
            .iter()
            .enumerate()
            .map(|(index, _)| {
                let id = self.fresh_type_param(owner_parameters.len() + index);
                bindings.insert(
                    shape
                        .identity_binder_at((owner_parameters.len() + index) as u32)
                        .expect("method binder"),
                    self.intern_type(hir::Type::Param(id)),
                );
                id
            })
            .collect::<Vec<_>>();
        let mut parameters = owner_parameters.to_vec();
        for (binder, id) in binders.iter().zip(ids) {
            parameters.push(
                self.resolve_imported_type_parameter(binder, id, &bindings, span)
                    .map_err(|error| Diagnostic::at(span, error))?,
            );
        }
        self.query_imported_method_parameters
            .insert(identity, parameters.clone());
        Ok(parameters)
    }
}
