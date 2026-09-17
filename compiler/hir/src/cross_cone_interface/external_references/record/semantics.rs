use std::fmt;

use scoop_identity::ConeIdentity;

use super::{ExternalHirReferenceV1, ExternalHirTargetV1};

/// Supplies the canonical owner Cone for a resolved external HIR target.
///
/// Implementations must derive the result from the target's kind-specific
/// canonical identity key. A name, table position, or untyped raw id is not
/// sufficient authority.
pub trait ExternalHirReferenceSemanticAuthority<E> {
    fn current_cone(&self) -> ConeIdentity;

    fn external_hir_target_origin(
        &mut self,
        target: ExternalHirTargetV1,
    ) -> Result<ConeIdentity, E>;
}

impl ExternalHirReferenceV1 {
    pub fn validate_semantics<A, E>(
        &self,
        authority: &mut A,
    ) -> Result<(), ExternalHirReferenceSemanticValidationError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let current = authority.current_cone();
        if self.origin() == current {
            return Err(
                ExternalHirReferenceSemanticValidationError::CurrentConeTarget {
                    target: self.target(),
                    current,
                },
            );
        }

        let expected = authority
            .external_hir_target_origin(self.target())
            .map_err(
                |error| ExternalHirReferenceSemanticValidationError::TargetOrigin {
                    target: self.target(),
                    error,
                },
            )?;
        if self.origin() != expected {
            return Err(
                ExternalHirReferenceSemanticValidationError::OriginMismatch {
                    target: self.target(),
                    expected,
                    actual: self.origin(),
                },
            );
        }
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExternalHirReferenceSemanticValidationError<E> {
    CurrentConeTarget {
        target: ExternalHirTargetV1,
        current: ConeIdentity,
    },
    TargetOrigin {
        target: ExternalHirTargetV1,
        error: E,
    },
    OriginMismatch {
        target: ExternalHirTargetV1,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
}

impl<E: fmt::Display> fmt::Display for ExternalHirReferenceSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentConeTarget { target, current } => write!(
                formatter,
                "external HIR target {target:?} belongs to current Cone {current}"
            ),
            Self::TargetOrigin { target, error } => write!(
                formatter,
                "external HIR target {target:?} origin is unavailable: {error}"
            ),
            Self::OriginMismatch {
                target,
                expected,
                actual,
            } => write!(
                formatter,
                "external HIR target {target:?} belongs to Cone {expected}, not recorded origin {actual}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExternalHirReferenceSemanticValidationError<E>
{
}

#[cfg(test)]
mod tests;
