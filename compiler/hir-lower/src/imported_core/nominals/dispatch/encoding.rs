use super::*;

impl Lowerer {
    pub(in crate::imported_core) fn resolve_imported_element_encoding(
        &mut self,
        receiver: hir::TypeId,
        declaration: &hir::ImportedNominalDeclaration,
    ) -> Result<Option<hir::ElementEncoding>, ImportedSignatureTypeError> {
        let Some(encoding) = declaration
            .interface
            .declaration_details()
            .element_encoding()
        else {
            return Ok(None);
        };
        let element = self.imported_dispatch_receiver(encoding.element(), receiver)?;
        let interface = self.imported_dispatch_receiver(encoding.interface(), receiver)?;
        let mut implementations =
            self.resolve_imported_interface_implementations(receiver, declaration, &[interface])?;
        if implementations.len() != 1 {
            return Err(ImportedSignatureTypeError::Structural);
        }
        Ok(Some(hir::ElementEncoding {
            element,
            implementation: implementations.remove(0),
        }))
    }
}
