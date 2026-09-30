//! Complete value representations obtained from dependency declarations.

use super::*;
use std::sync::Arc;

mod classes;
mod dispatch;
mod enums;
mod source;
mod structs;

impl Lowerer {
    pub(super) fn imported_nominal_type(
        &mut self,
        identity: PersistentTypeId,
    ) -> Result<hir::TypeId, ImportedSignatureTypeError> {
        self.imported_nominal_application(hir::SourceNominalId::Concrete(identity), Vec::new())
    }

    pub(crate) fn imported_nominal_application(
        &mut self,
        owner: hir::SourceNominalId,
        arguments: Vec<hir::TypeId>,
    ) -> Result<hir::TypeId, ImportedSignatureTypeError> {
        if let Some(application) = self
            .struct_application_by_key
            .get(&(owner, arguments.clone()))
        {
            return Ok(self.struct_applications[*application].canonical_type);
        }
        if let Some(application) = self
            .enum_application_by_key
            .get(&(owner, arguments.clone()))
        {
            return Ok(self.enum_applications[*application].canonical_type);
        }
        if let Some(application) = self
            .class_application_by_key
            .get(&(owner, arguments.clone()))
        {
            return Ok(self.class_applications[*application].canonical_type);
        }
        if let Some((id, _)) = self.types.iter().find(|(_, ty)| {
            ty.imported_nominal_application()
                .is_some_and(|(declaration, existing)| {
                    declaration.owner() == owner && existing == arguments
                })
        }) {
            return Ok(id);
        }
        let declaration = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.nominal_declaration(owner))
            .cloned()
            .ok_or(ImportedSignatureTypeError::Structural)?;
        if arguments.len() != declaration.interface.type_parameters().binders().len() {
            return Err(ImportedSignatureTypeError::Structural);
        }
        let bindings = arguments
            .iter()
            .enumerate()
            .map(|(index, ty)| {
                (
                    SignatureTypeKey::Binder {
                        depth: 0,
                        index: index as u32,
                    },
                    *ty,
                )
            })
            .collect();
        match declaration.interface.source_shape() {
            hir::NominalSourceShapeV1::Class(_) | hir::NominalSourceShapeV1::Object(_) => {
                self.imported_class_type(declaration, arguments)
            }
            hir::NominalSourceShapeV1::Interface => {
                self.imported_interface_type(declaration, arguments, &bindings)
            }
            hir::NominalSourceShapeV1::Struct(_) => {
                self.imported_struct_type(declaration, arguments)
            }
            hir::NominalSourceShapeV1::Enum(_) => self.imported_enum_type(declaration, arguments),
            hir::NominalSourceShapeV1::Intrinsic(representation)
                if representation.family() == hir::IntrinsicTypeKind::Ptr =>
            {
                Ok(self.intern_type(hir::Type::Ptr(arguments[0])))
            }
            hir::NominalSourceShapeV1::Intrinsic(representation)
                if matches!(
                    representation.family(),
                    hir::IntrinsicTypeKind::Array | hir::IntrinsicTypeKind::MutableArray
                ) =>
            {
                self.imported_class_type(declaration, arguments)
            }
            hir::NominalSourceShapeV1::Intrinsic(_) => Err(ImportedSignatureTypeError::Structural),
        }
    }
}
