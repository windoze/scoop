//! Resolve dependency value fields and conformance using their real identities.

use super::*;

impl Lowerer {
    pub(super) fn imported_struct_type(
        &mut self,
        declaration: Arc<hir::ImportedNominalDeclaration>,
        arguments: Vec<hir::TypeId>,
        bindings: &ImportedTypeBindings,
    ) -> Result<hir::TypeId, ImportedSignatureTypeError> {
        let hir::NominalSourceShapeV1::Struct(shape) = declaration.interface.source_shape() else {
            return Err(ImportedSignatureTypeError::Structural);
        };
        // Register before resolving fields or interface signatures, either of
        // which can refer back through a reference type to this exact value.
        let mut structure = hir::ImportedStructType {
            declaration: Arc::clone(&declaration),
            arguments,
            fields: Vec::new(),
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
        };
        let ty = self.intern_type(hir::Type::ImportedStruct(Arc::new(structure.clone())));
        structure.fields = shape
            .fields()
            .iter()
            .zip(&declaration.field_sources)
            .map(|(field, source)| {
                Ok(hir::ImportedNominalField {
                    identity: field.field(),
                    name: source.name.clone(),
                    ty: self.imported_signature_type_with_bindings(field.value_type(), bindings)?,
                })
            })
            .collect::<Result<Vec<_>, ImportedSignatureTypeError>>()?;
        // An interface method can contain another value using this payload.
        // Publish the completed fields before resolving those method types.
        self.types[ty] = hir::Type::ImportedStruct(Arc::new(structure.clone()));
        structure.interfaces = self.imported_value_interfaces(&declaration, bindings)?;
        self.types[ty] = hir::Type::ImportedStruct(Arc::new(structure.clone()));
        structure.interface_implementations = self.resolve_imported_interface_implementations(
            ty,
            &declaration,
            &structure.interfaces,
        )?;
        self.types[ty] = hir::Type::ImportedStruct(Arc::new(structure));
        Ok(ty)
    }

    fn imported_value_interfaces(
        &mut self,
        declaration: &hir::ImportedNominalDeclaration,
        bindings: &ImportedTypeBindings,
    ) -> Result<Vec<hir::TypeId>, ImportedSignatureTypeError> {
        declaration
            .interface
            .exact_supertypes()
            .values()
            .iter()
            .map(|parent| self.imported_signature_type_with_bindings(parent, bindings))
            .collect()
    }
}
