//! Complete value representations obtained from dependency declarations.

mod context;
use super::*;
use std::sync::Arc;

mod classes;
mod dispatch;
mod enums;
mod source;
mod structs;

impl Lowerer {
    pub(super) fn decoded_nominal_pointee_requirements(
        declaration: &hir::ImportedNominalDeclaration,
        parameters: &[hir::TypeParamDecl],
    ) -> Vec<hir::RequiresGcFreePointee> {
        declaration
            .interface
            .declaration_details()
            .instantiation_conditions()
            .gc_free_pointees()
            .arguments()
            .iter()
            .map(|argument| {
                let SignatureTypeKey::Binder { depth: 0, index } = *argument else {
                    unreachable!("decoded nominal conditions name original binders")
                };
                hir::RequiresGcFreePointee {
                    type_param: parameters[index as usize].id,
                }
            })
            .collect()
    }

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
        if let Some(application) = self
            .interface_application_by_key
            .get(&(owner, arguments.clone()))
        {
            return Ok(self.interface_applications[*application].canonical_type);
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
        match declaration.interface.source_shape() {
            hir::NominalSourceShapeV1::Class(_) | hir::NominalSourceShapeV1::Object(_) => {
                self.imported_class_type(declaration, arguments)
            }
            hir::NominalSourceShapeV1::Interface => {
                self.imported_interface_type(declaration, arguments)
            }
            hir::NominalSourceShapeV1::Intrinsic(representation)
                if matches!(
                    representation.family(),
                    hir::IntrinsicTypeKind::Char | hir::IntrinsicTypeKind::Float(_)
                ) =>
            {
                self.imported_struct_type(declaration, arguments)
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
                if representation.family() == hir::IntrinsicTypeKind::FunPtr =>
            {
                if !matches!(
                    self.types[arguments[0]],
                    hir::Type::Function(_) | hir::Type::Param(_)
                ) {
                    return Err(ImportedSignatureTypeError::Structural);
                }
                self.imported_struct_type(declaration, arguments)
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
