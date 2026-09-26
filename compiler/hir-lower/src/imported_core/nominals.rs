//! Complete value representations obtained from dependency declarations.

use super::*;
use std::sync::Arc;

impl Lowerer {
    pub(super) fn imported_nominal_type(
        &mut self,
        identity: PersistentTypeId,
    ) -> Result<hir::TypeId, ImportedSignatureTypeError> {
        if let Some((id, _)) = self.types.iter().find(|(_, ty)| match ty {
            hir::Type::ImportedStruct(ty) => ty.declaration.identity.id() == identity,
            hir::Type::ImportedEnum(ty) => ty.declaration.identity.id() == identity,
            _ => false,
        }) {
            return Ok(id);
        }
        let declaration = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.nominal(identity))
            .cloned()
            .ok_or(ImportedSignatureTypeError::Structural)?;
        for parent in declaration.interface.exact_supertypes().values() {
            self.imported_signature_type(parent)?;
        }
        match declaration.interface.source_shape() {
            hir::NominalSourceShapeV1::Struct(shape) => {
                let fields = shape
                    .fields()
                    .iter()
                    .zip(&declaration.field_names)
                    .map(|(field, name)| {
                        Ok(hir::ImportedStructField {
                            identity: field.field(),
                            name: name.clone(),
                            ty: self.imported_signature_type(field.value_type())?,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let gc_free = fields.iter().all(|field| self.is_gc_free(field.ty));
                Ok(self.intern_type(hir::Type::ImportedStruct(Arc::new(
                    hir::ImportedStructType {
                        declaration,
                        fields,
                        gc_free,
                    },
                ))))
            }
            hir::NominalSourceShapeV1::Enum(shape) => {
                let variants = shape
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
                            .collect::<Result<Vec<_>, _>>()?;
                        Ok(hir::ImportedEnumValueVariant {
                            identity: variant.variant(),
                            name: names.name.clone(),
                            style: variant.style(),
                            fields,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let gc_free = variants
                    .iter()
                    .flat_map(|variant| &variant.fields)
                    .all(|field| self.is_gc_free(field.ty));
                Ok(
                    self.intern_type(hir::Type::ImportedEnum(Arc::new(hir::ImportedEnumType {
                        declaration,
                        variants,
                        gc_free,
                    }))),
                )
            }
            _ => Err(ImportedSignatureTypeError::Structural),
        }
    }
}
