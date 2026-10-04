//! Uncalled member contracts still belong to a concrete dependency owner.

use super::*;
use hir::ImportedCallableSource;

impl Lowerer {
    pub(super) fn imported_nominal_contexts(
        &mut self,
        owner: hir::SourceNominalId,
        bindings: &ImportedTypeBindings,
    ) -> Result<Vec<hir::LoadedContextContract>, ImportedSignatureTypeError> {
        let declarations = self
            .dependencies
            .as_ref()
            .ok_or(ImportedSignatureTypeError::Structural)?
            .nominal_context_declarations(owner)
            .map_err(|_| ImportedSignatureTypeError::Structural)?;
        declarations
            .iter()
            .map(|declaration| {
                let interface = declaration.interface();
                let identity = match interface.declaration() {
                    scoop_identity::CallableTemplateOrigin::Function(id) => {
                        hir::DefaultCallableDeclarationV1::Function(id)
                    }
                    scoop_identity::CallableTemplateOrigin::GenericFunction(id) => {
                        hir::DefaultCallableDeclarationV1::GenericFunction(id)
                    }
                    scoop_identity::CallableTemplateOrigin::Accessor(id) => {
                        hir::DefaultCallableDeclarationV1::PropertyAccessor(id)
                    }
                    _ => return Err(ImportedSignatureTypeError::Structural),
                };
                let parameters = interface
                    .context_parameters()
                    .iter()
                    .map(|p| self.imported_signature_type_with_bindings(p.value_type(), bindings))
                    .collect::<Result<_, _>>()?;
                Ok(hir::LoadedContextContract {
                    declaration: identity,
                    name: declaration.name().to_owned(),
                    parameters,
                })
            })
            .collect()
    }
}
