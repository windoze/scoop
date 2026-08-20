use std::fmt;

use scoop_identity::{BindableEntity, PersistentExportBindingId};

use super::HirExportBindingIdentities;
use crate::{CanonicalPublicExportBindingsV1, ExportBindingSourceV1};

impl HirExportBindingIdentities {
    /// Proves that every persistent export binding has exactly one public
    /// source record and that declared-current records name their key target.
    pub fn validate_public_bindings(
        &self,
        surface: &CanonicalPublicExportBindingsV1,
    ) -> Result<(), HirExportBindingSurfaceValidationError> {
        if self.records.len() != surface.records().len() {
            return Err(HirExportBindingSurfaceValidationError::Length {
                identities: self.records.len(),
                surface: surface.records().len(),
            });
        }
        for (index, (identity, record)) in self.records.iter().zip(surface.records()).enumerate() {
            if identity.id() != record.binding() {
                return Err(HirExportBindingSurfaceValidationError::IdentityMismatch {
                    index,
                    identity: identity.id(),
                    surface: record.binding(),
                });
            }
            if let ExportBindingSourceV1::DeclaredCurrent { declaration } = record.source()
                && identity.key().target() != *declaration
            {
                return Err(
                    HirExportBindingSurfaceValidationError::DeclarationMismatch {
                        index,
                        binding: identity.id(),
                        expected: identity.key().target(),
                        actual: *declaration,
                    },
                );
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirExportBindingSurfaceValidationError {
    Length {
        identities: usize,
        surface: usize,
    },
    IdentityMismatch {
        index: usize,
        identity: PersistentExportBindingId,
        surface: PersistentExportBindingId,
    },
    DeclarationMismatch {
        index: usize,
        binding: PersistentExportBindingId,
        expected: BindableEntity,
        actual: BindableEntity,
    },
}

impl fmt::Display for HirExportBindingSurfaceValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length {
                identities,
                surface,
            } => write!(
                formatter,
                "export binding identity count {identities} differs from public surface count {surface}"
            ),
            Self::IdentityMismatch {
                index,
                identity,
                surface,
            } => write!(
                formatter,
                "export binding identity {identity} at index {index} differs from public surface binding {surface}"
            ),
            Self::DeclarationMismatch {
                index,
                binding,
                expected,
                actual,
            } => write!(
                formatter,
                "declared-current export binding {binding} at index {index} names {actual:?}, expected {expected:?}"
            ),
        }
    }
}

impl std::error::Error for HirExportBindingSurfaceValidationError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
        DefinitionOwnerChain, ExportBindingKey, PackagePath, PersistentExportBindingId,
        PersistentFunctionId, SourceDeclarationKey, SourceDeclarationSite,
    };

    use super::*;
    use crate::PublicExportBindingRecordV1;

    #[test]
    fn public_binding_surface_requires_exact_identity_alignment() {
        let first = binding("first");
        let second = binding("second");
        let identities = HirExportBindingIdentities::canonicalize(vec![first.clone()]).unwrap();

        let empty = CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap();
        assert!(matches!(
            identities.validate_public_bindings(&empty),
            Err(HirExportBindingSurfaceValidationError::Length {
                identities: 1,
                surface: 0
            })
        ));

        let mismatched = CanonicalPublicExportBindingsV1::try_new(vec![declared(&second)]).unwrap();
        assert!(matches!(
            identities.validate_public_bindings(&mismatched),
            Err(HirExportBindingSurfaceValidationError::IdentityMismatch { index: 0, .. })
        ));
    }

    #[test]
    fn declared_current_source_must_name_the_binding_target() {
        let first = binding("first");
        let second = binding("second");
        let identities = HirExportBindingIdentities::canonicalize(vec![first.clone()]).unwrap();
        let surface =
            CanonicalPublicExportBindingsV1::try_new(vec![PublicExportBindingRecordV1::new(
                first.id(),
                ExportBindingSourceV1::DeclaredCurrent {
                    declaration: second.key().target(),
                },
            )])
            .unwrap();

        assert!(matches!(
            identities.validate_public_bindings(&surface),
            Err(HirExportBindingSurfaceValidationError::DeclarationMismatch { index: 0, .. })
        ));
    }

    fn declared(
        binding: &CborIdentityRecord<PersistentExportBindingId, ExportBindingKey>,
    ) -> PublicExportBindingRecordV1 {
        PublicExportBindingRecordV1::new(
            binding.id(),
            ExportBindingSourceV1::DeclaredCurrent {
                declaration: binding.key().target(),
            },
        )
    }

    fn binding(name: &str) -> CborIdentityRecord<PersistentExportBindingId, ExportBindingKey> {
        let declaration: CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey> =
            CborIdentityRecord::from_key(SourceDeclarationKey::function(
                SourceDeclarationSite::new(
                    ConeIdentity::CORE,
                    PackagePath::root(),
                    DefinitionOwnerChain::top_level(),
                    DeclarationScope::ConeWide,
                )
                .unwrap(),
                CanonicalIdentifier::new(name).unwrap(),
                0,
                None,
                Vec::new(),
            ))
            .unwrap();
        CborIdentityRecord::from_key(ExportBindingKey::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            CanonicalIdentifier::new(name).unwrap(),
            BindingTarget::function(declaration.key()).unwrap(),
        ))
        .unwrap()
    }
}
