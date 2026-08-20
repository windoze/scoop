use std::fmt;

use scoop_identity::PersistentExportBindingId;

use super::{CanonicalPublicExportBindingsV1, ExportBindingSourceV1};
use crate::CanonicalDirectPublicSurfaceV1;

impl CanonicalPublicExportBindingsV1 {
    /// Proves that the declared-current subset is byte-for-byte identical to
    /// the already validated foundation direct-public inventory. Re-exports
    /// remain part of this table but can never satisfy a direct entry.
    pub fn validate_direct_surface(
        &self,
        direct_surface: &CanonicalDirectPublicSurfaceV1,
    ) -> Result<(), PublicExportBindingDirectSurfaceValidationError> {
        for (surface_index, binding) in direct_surface.bindings().iter().copied().enumerate() {
            let record_index = self
                .records()
                .binary_search_by_key(&binding, |record| record.binding())
                .map_err(|insertion_index| {
                    PublicExportBindingDirectSurfaceValidationError::MissingDeclaredCurrent {
                        surface_index,
                        insertion_index,
                        binding,
                    }
                })?;
            if matches!(
                self.records()[record_index].source(),
                ExportBindingSourceV1::Reexport { .. }
            ) {
                return Err(
                    PublicExportBindingDirectSurfaceValidationError::DirectBindingIsReexport {
                        surface_index,
                        record_index,
                        binding,
                    },
                );
            }
        }

        for (record_index, record) in self.records().iter().enumerate() {
            if !matches!(
                record.source(),
                ExportBindingSourceV1::DeclaredCurrent { .. }
            ) {
                continue;
            }
            if let Err(insertion_index) = direct_surface.bindings().binary_search(&record.binding())
            {
                return Err(
                    PublicExportBindingDirectSurfaceValidationError::UnexpectedDeclaredCurrent {
                        record_index,
                        insertion_index,
                        binding: record.binding(),
                    },
                );
            }
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicExportBindingDirectSurfaceValidationError {
    MissingDeclaredCurrent {
        surface_index: usize,
        insertion_index: usize,
        binding: PersistentExportBindingId,
    },
    DirectBindingIsReexport {
        surface_index: usize,
        record_index: usize,
        binding: PersistentExportBindingId,
    },
    UnexpectedDeclaredCurrent {
        record_index: usize,
        insertion_index: usize,
        binding: PersistentExportBindingId,
    },
}

impl fmt::Display for PublicExportBindingDirectSurfaceValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDeclaredCurrent {
                surface_index,
                insertion_index,
                binding,
            } => write!(
                formatter,
                "foundation direct-public binding {binding} at index {surface_index} is absent from the cross-Cone surface at canonical insertion index {insertion_index}"
            ),
            Self::DirectBindingIsReexport {
                surface_index,
                record_index,
                binding,
            } => write!(
                formatter,
                "foundation direct-public binding {binding} at index {surface_index} is a re-export at cross-Cone surface index {record_index}"
            ),
            Self::UnexpectedDeclaredCurrent {
                record_index,
                insertion_index,
                binding,
            } => write!(
                formatter,
                "declared-current binding {binding} at cross-Cone surface index {record_index} is absent from the foundation direct-public surface at canonical insertion index {insertion_index}"
            ),
        }
    }
}

impl std::error::Error for PublicExportBindingDirectSurfaceValidationError {}
