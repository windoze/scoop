//! Complete value representations obtained from dependency declarations.

use super::*;
use std::sync::Arc;

mod dispatch;
mod source;
mod values;

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
                self.imported_class_type(declaration, arguments, &bindings)
            }
            hir::NominalSourceShapeV1::Interface => {
                self.imported_interface_type(declaration, arguments, &bindings)
            }
            hir::NominalSourceShapeV1::Struct(_) => {
                self.imported_struct_type(declaration, arguments, &bindings)
            }
            hir::NominalSourceShapeV1::Enum(_) => {
                self.imported_enum_type(declaration, arguments, &bindings)
            }
            hir::NominalSourceShapeV1::Intrinsic(representation)
                if matches!(
                    representation.family(),
                    hir::IntrinsicTypeKind::Array | hir::IntrinsicTypeKind::MutableArray
                ) =>
            {
                self.imported_class_type(declaration, arguments, &bindings)
            }
            hir::NominalSourceShapeV1::Intrinsic(_) => Err(ImportedSignatureTypeError::Structural),
        }
    }

    fn imported_class_type(
        &mut self,
        declaration: Arc<hir::ImportedNominalDeclaration>,
        arguments: Vec<hir::TypeId>,
        bindings: &ImportedTypeBindings,
    ) -> Result<hir::TypeId, ImportedSignatureTypeError> {
        // Register the reference identity before resolving fields, which may refer
        // back to this class. Complete the record before any HIR can be emitted.
        let mut class = hir::ImportedClassType {
            declaration: Arc::clone(&declaration),
            arguments,
            fields: Vec::new(),
            base_class: None,
            interfaces: Vec::new(),
            virtual_methods: Vec::new(),
            interface_implementations: Vec::new(),
        };
        let ty = self.intern_type(hir::Type::ImportedClass(Arc::new(class.clone())));
        for parent in declaration.interface.exact_supertypes().values() {
            let parent = self.imported_signature_type_with_bindings(parent, bindings)?;
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
            .zip(&declaration.field_sources)
            .map(|(field, source)| {
                Ok(hir::ImportedNominalField {
                    identity: field.field(),
                    name: source.name.clone(),
                    ty: self.imported_signature_type_with_bindings(field.value_type(), bindings)?,
                })
            })
            .collect::<Result<Vec<_>, ImportedSignatureTypeError>>()?;
        self.resolve_imported_class_dispatch(&mut class)?;
        self.types[ty] = hir::Type::ImportedClass(Arc::new(class));
        Ok(ty)
    }
}
