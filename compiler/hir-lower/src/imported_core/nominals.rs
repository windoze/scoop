//! Complete value representations obtained from dependency declarations.

use super::*;
use std::sync::Arc;

mod values;

impl Lowerer {
    pub(super) fn imported_nominal_type(
        &mut self,
        identity: PersistentTypeId,
    ) -> Result<hir::TypeId, ImportedSignatureTypeError> {
        if let Some((id, _)) = self.types.iter().find(|(_, ty)| match ty {
            hir::Type::ImportedStruct(ty) => ty.declaration.identity.id() == identity,
            hir::Type::ImportedEnum(ty) => ty.declaration.identity.id() == identity,
            hir::Type::ImportedClass(ty) => ty.declaration.identity.id() == identity,
            hir::Type::ImportedInterface(ty) => ty.declaration.identity.id() == identity,
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
        if matches!(
            declaration.interface.source_shape(),
            hir::NominalSourceShapeV1::Class(_) | hir::NominalSourceShapeV1::Object(_)
        ) {
            return self.imported_class_type(declaration);
        }
        if matches!(
            declaration.interface.source_shape(),
            hir::NominalSourceShapeV1::Interface
        ) {
            return self.imported_interface_type(declaration);
        }
        match declaration.interface.source_shape() {
            hir::NominalSourceShapeV1::Struct(_) => self.imported_struct_type(declaration),
            hir::NominalSourceShapeV1::Enum(_) => self.imported_enum_type(declaration),
            _ => Err(ImportedSignatureTypeError::Structural),
        }
    }

    fn imported_class_type(
        &mut self,
        declaration: Arc<hir::ImportedNominalDeclaration>,
    ) -> Result<hir::TypeId, ImportedSignatureTypeError> {
        // Register the reference identity before resolving fields, which may refer
        // back to this class. Complete the record before any HIR can be emitted.
        let mut class = hir::ImportedClassType {
            declaration: Arc::clone(&declaration),
            fields: Vec::new(),
            base_class: None,
            interfaces: Vec::new(),
            virtual_slots: Vec::new(),
        };
        let ty = self.intern_type(hir::Type::ImportedClass(Arc::new(class.clone())));
        for parent in declaration.interface.exact_supertypes().values() {
            let parent = self.imported_signature_type(parent)?;
            match &self.types[parent] {
                hir::Type::ImportedClass(_) | hir::Type::Class(_) => {
                    class.base_class = Some(parent)
                }
                _ => class.interfaces.push(parent),
            }
        }
        class.fields = declaration
            .interface
            .source_shape()
            .declared_fields()
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
        if let Some(base) = class.base_class {
            let hir::Type::ImportedClass(base) = &self.types[base] else {
                return Err(ImportedSignatureTypeError::Structural);
            };
            class.virtual_slots = base.virtual_slots.clone();
        }
        for slot in declaration
            .interface
            .declaration_details()
            .dispatch_order()
            .declared_slots()
        {
            if !class.virtual_slots.contains(&slot) {
                class.virtual_slots.push(slot);
            }
        }
        self.types[ty] = hir::Type::ImportedClass(Arc::new(class));
        Ok(ty)
    }
}
