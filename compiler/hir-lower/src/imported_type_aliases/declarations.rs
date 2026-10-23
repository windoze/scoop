use super::*;

impl Lowerer {
    pub(crate) fn imported_alias_declaration_types(
        &mut self,
        declaration: &hir::TypeAliasInterfaceRecordV1,
        expansion: &scoop_identity::SignatureTypeKey,
        span: ast::Span,
    ) -> Result<(Vec<hir::TypeParamDecl>, hir::TypeId), ast::Diagnostic> {
        let parameters = declaration
            .type_parameters()
            .binders()
            .iter()
            .collect::<Vec<_>>();
        let signatures = (0..parameters.len())
            .map(|index| scoop_identity::SignatureTypeKey::Binder {
                depth: 0,
                index: index as u32,
            })
            .collect::<Vec<_>>();
        let (parameters, bindings) = self
            .prepare_imported_type_parameters(&parameters, &signatures, span)
            .map_err(|error| {
                ast::Diagnostic::at(
                    span,
                    format!("invalid imported typealias parameters: {error}"),
                )
            })?;
        let target = self
            .imported_signature_type_with_bindings(expansion, &bindings)
            .map_err(|error| {
                ast::Diagnostic::at(
                    span,
                    format!("invalid imported typealias target: {error:?}"),
                )
            })?;
        Ok((parameters, target))
    }
}
