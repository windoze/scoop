//! Source declarations behind imported primitive representations.

use super::*;

impl Lowerer {
    pub(crate) fn require_imported_bound_interface(
        &mut self,
        interface: hir::TypeId,
    ) -> Result<(), ImportedSignatureTypeError> {
        let Some(interface) = self.dependency_interface_definition(interface) else {
            return Err(ImportedSignatureTypeError::Structural);
        };
        if self
            .imported_bound_interfaces
            .insert(interface.declaration.owner())
        {
            let kinds = self
                .imported_intrinsic_types
                .keys()
                .copied()
                .collect::<Vec<_>>();
            for kind in kinds {
                self.resolve_imported_intrinsic_bound_conformances(kind)?;
            }
        }
        Ok(())
    }

    pub(crate) fn resolve_imported_member_receiver(
        &mut self,
        ty: hir::TypeId,
        span: Span,
    ) -> Result<(), ()> {
        self.resolve_imported_member_receiver_type(ty)
            .map_err(|error| self.error(span, error.diagnostic("member receiver")))
    }

    pub(crate) fn resolve_imported_member_receiver_type(
        &mut self,
        ty: hir::TypeId,
    ) -> Result<(), ImportedSignatureTypeError> {
        let kind = match self.types[ty] {
            hir::Type::Integer(kind) => hir::IntrinsicTypeKind::Integer(kind),
            hir::Type::Unit => hir::IntrinsicTypeKind::Unit,
            hir::Type::Boolean => hir::IntrinsicTypeKind::Boolean,
            hir::Type::String => hir::IntrinsicTypeKind::String,
            hir::Type::Any => hir::IntrinsicTypeKind::Any,
            _ => return Ok(()),
        };
        self.resolve_imported_intrinsic_type(kind)
    }

    pub(crate) fn retain_imported_box_source(
        &mut self,
        ty: hir::TypeId,
    ) -> Result<(), ImportedSignatureTypeError> {
        let kind = match self.types[ty] {
            hir::Type::Integer(kind) => hir::IntrinsicTypeKind::Integer(kind),
            hir::Type::Unit => hir::IntrinsicTypeKind::Unit,
            hir::Type::Boolean => hir::IntrinsicTypeKind::Boolean,
            _ => return Ok(()),
        };
        self.resolve_imported_intrinsic_type(kind)
    }

    pub(crate) fn resolve_imported_intrinsic_type(
        &mut self,
        kind: hir::IntrinsicTypeKind,
    ) -> Result<(), ImportedSignatureTypeError> {
        if self.imported_intrinsic_types.contains_key(&kind) {
            return Ok(());
        }
        let CoreLoweringAuthority::Imported(protocols) = &self.core else {
            return Ok(());
        };
        let fundamental = protocols.fundamental_types();
        let identity = match kind {
            hir::IntrinsicTypeKind::Integer(kind) => fundamental.integer(kind).persistent(),
            hir::IntrinsicTypeKind::Unit => fundamental.unit().persistent(),
            hir::IntrinsicTypeKind::Boolean => fundamental.boolean().persistent(),
            hir::IntrinsicTypeKind::String => fundamental.string().persistent(),
            hir::IntrinsicTypeKind::Any => fundamental.any().persistent(),
            hir::IntrinsicTypeKind::Nothing => fundamental.nothing().persistent(),
            hir::IntrinsicTypeKind::Float(_)
            | hir::IntrinsicTypeKind::Char
            | hir::IntrinsicTypeKind::Array
            | hir::IntrinsicTypeKind::MutableArray
            | hir::IntrinsicTypeKind::Ptr
            | hir::IntrinsicTypeKind::FunPtr => return Err(ImportedSignatureTypeError::Generic),
        };
        let declaration = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.nominal(identity))
            .cloned()
            .ok_or(ImportedSignatureTypeError::Structural)?;
        let interfaces = declaration
            .interface
            .exact_supertypes()
            .values()
            .iter()
            .map(|parent| self.imported_signature_type(parent))
            .collect::<Result<Vec<_>, _>>()?;
        self.imported_intrinsic_types.insert(
            kind,
            hir::ImportedIntrinsicType {
                declaration: declaration.clone(),
                interfaces: interfaces.clone(),
                interface_implementations: Vec::new(),
            },
        );
        self.resolve_imported_intrinsic_bound_conformances(kind)
    }

    fn resolve_imported_intrinsic_bound_conformances(
        &mut self,
        kind: hir::IntrinsicTypeKind,
    ) -> Result<(), ImportedSignatureTypeError> {
        let source = &self.imported_intrinsic_types[&kind];
        if !source.interface_implementations.is_empty()
            || !source.interfaces.iter().any(|interface| {
                self.dependency_interface_definition(*interface)
                    .is_some_and(|interface| {
                        self.imported_bound_interfaces
                            .contains(&interface.declaration.owner())
                    })
            })
        {
            return Ok(());
        }
        let declaration = source.declaration.clone();
        let interfaces = source.interfaces.clone();
        let ty = match kind {
            hir::IntrinsicTypeKind::Integer(kind) => self.intern_type(hir::Type::Integer(kind)),
            hir::IntrinsicTypeKind::Unit => self.unit,
            hir::IntrinsicTypeKind::Boolean => self.boolean,
            hir::IntrinsicTypeKind::String => self.intern_type(hir::Type::String),
            hir::IntrinsicTypeKind::Any => self.any,
            _ => unreachable!("non-generic intrinsic declarations are scalar types"),
        };
        let implementations =
            self.resolve_imported_interface_implementations(ty, &declaration, &interfaces)?;
        self.imported_intrinsic_types
            .get_mut(&kind)
            .expect("an intrinsic declaration was reserved before its conformance")
            .interface_implementations = implementations;
        Ok(())
    }
}

impl ImportedSignatureTypeError {
    pub(crate) fn diagnostic(self, subject: &str) -> String {
        match self {
            Self::Generic => crate::imported_capabilities::ImportedCapabilityRequirement::Generic
                .diagnostic(subject),
            Self::Structural => {
                format!("cannot resolve {subject} from dependency declarations")
            }
        }
    }
}
