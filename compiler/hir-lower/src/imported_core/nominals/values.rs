//! Resolve dependency value fields and conformance using their real identities.

use super::*;

impl Lowerer {
    pub(super) fn imported_struct_type(
        &mut self,
        declaration: Arc<hir::ImportedNominalDeclaration>,
    ) -> Result<hir::TypeId, ImportedSignatureTypeError> {
        let hir::NominalSourceShapeV1::Struct(shape) = declaration.interface.source_shape() else {
            return Err(ImportedSignatureTypeError::Structural);
        };
        // Register before resolving fields or interface signatures, either of
        // which can refer back through a reference type to this exact value.
        let mut structure = hir::ImportedStructType {
            declaration: Arc::clone(&declaration),
            fields: Vec::new(),
            interfaces: Vec::new(),
            gc_free: false,
        };
        let ty = self.intern_type(hir::Type::ImportedStruct(Arc::new(structure.clone())));
        structure.fields = shape
            .fields()
            .iter()
            .zip(&declaration.field_names)
            .map(|(field, name)| {
                Ok(hir::ImportedNominalField {
                    identity: field.field(),
                    name: name.clone(),
                    ty: self.imported_signature_type(field.value_type())?,
                })
            })
            .collect::<Result<Vec<_>, ImportedSignatureTypeError>>()?;
        structure.gc_free = structure
            .fields
            .iter()
            .all(|field| self.is_gc_free(field.ty));
        // An interface method can contain another value using this payload.
        // Publish the completed fields before resolving those method types.
        self.types[ty] = hir::Type::ImportedStruct(Arc::new(structure.clone()));
        structure.interfaces = self.imported_value_interfaces(&declaration)?;
        self.types[ty] = hir::Type::ImportedStruct(Arc::new(structure));
        Ok(ty)
    }

    pub(super) fn imported_enum_type(
        &mut self,
        declaration: Arc<hir::ImportedNominalDeclaration>,
    ) -> Result<hir::TypeId, ImportedSignatureTypeError> {
        let hir::NominalSourceShapeV1::Enum(shape) = declaration.interface.source_shape() else {
            return Err(ImportedSignatureTypeError::Structural);
        };
        let mut enumeration = hir::ImportedEnumType {
            declaration: Arc::clone(&declaration),
            variants: Vec::new(),
            interfaces: Vec::new(),
            gc_free: false,
        };
        let ty = self.intern_type(hir::Type::ImportedEnum(Arc::new(enumeration.clone())));
        enumeration.variants = shape
            .variants()
            .iter()
            .zip(&declaration.variant_names)
            .map(|(variant, names)| {
                let fields = variant
                    .fields()
                    .iter()
                    .zip(&names.fields)
                    .map(|(field, name)| {
                        Ok(hir::ImportedEnumField {
                            identity: field.field(),
                            name: name.clone(),
                            ty: self.imported_signature_type(field.value_type())?,
                        })
                    })
                    .collect::<Result<Vec<_>, ImportedSignatureTypeError>>()?;
                Ok(hir::ImportedEnumValueVariant {
                    identity: variant.variant(),
                    name: names.name.clone(),
                    style: variant.style(),
                    fields,
                })
            })
            .collect::<Result<Vec<_>, ImportedSignatureTypeError>>()?;
        enumeration.gc_free = enumeration
            .variants
            .iter()
            .flat_map(|variant| &variant.fields)
            .all(|field| self.is_gc_free(field.ty));
        self.types[ty] = hir::Type::ImportedEnum(Arc::new(enumeration.clone()));
        enumeration.interfaces = self.imported_value_interfaces(&declaration)?;
        self.types[ty] = hir::Type::ImportedEnum(Arc::new(enumeration));
        Ok(ty)
    }

    fn imported_value_interfaces(
        &mut self,
        declaration: &hir::ImportedNominalDeclaration,
    ) -> Result<Vec<hir::TypeId>, ImportedSignatureTypeError> {
        declaration
            .interface
            .exact_supertypes()
            .values()
            .iter()
            .map(|parent| self.imported_signature_type(parent))
            .collect()
    }
}
