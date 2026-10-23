//! Restore conditional relations without strengthening nominal application bounds.

use super::*;

impl Lowerer {
    pub(super) fn load_imported_conditional_interfaces(
        &mut self,
        declaration: &hir::ImportedNominalDeclaration,
        parameters: &[hir::TypeParamDecl],
        bindings: &ImportedTypeBindings,
        span: Span,
    ) -> Result<Vec<hir::ConditionalInterfaceDeclaration>, ImportedSignatureTypeError> {
        declaration
            .interface
            .declaration_details()
            .conditional_interfaces()
            .iter()
            .map(|implementation| {
                let parameters = implementation
                    .parameters()
                    .binders()
                    .iter()
                    .zip(parameters)
                    .map(|(binder, parameter)| {
                        self.resolve_imported_type_parameter(binder, parameter.id, bindings, span)
                            .map_err(|_| ImportedSignatureTypeError::Structural)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(hir::ConditionalInterfaceDeclaration {
                    key: implementation.key(declaration.owner()),
                    interface: self.imported_signature_type_with_bindings(
                        implementation.interface(),
                        bindings,
                    )?,
                    parameters,
                    span,
                })
            })
            .collect()
    }
}
