use std::fmt;

use scoop_wire::{BudgetMeter, WireError, WirePath};

use scoop_identity::{BindingTarget, ConeIdentity};

use super::{ExternalHirReferenceV1, ExternalHirTargetV1};
use crate::{DependencyBindingWitnessSemanticValidationError, PublicExportBindingClosureAuthority};

/// Supplies the canonical owner Cone for a resolved external HIR target.
///
/// Implementations must derive the result from the target's kind-specific
/// canonical identity key. A name, table position, or untyped raw id is not
/// sufficient authority.
pub trait ExternalHirReferenceSemanticAuthority<E>: PublicExportBindingClosureAuthority {
    fn current_cone(&self) -> ConeIdentity;

    fn external_hir_target_origin(
        &mut self,
        target: ExternalHirTargetV1,
    ) -> Result<ConeIdentity, E>;

    fn external_hir_target_binding_root(
        &mut self,
        target: ExternalHirTargetV1,
    ) -> Result<BindingTarget, E>;
}

impl ExternalHirReferenceV1 {
    pub fn validate_semantics<A, E>(
        &self,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), ExternalHirReferenceSemanticValidationError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        meter
            .charge_nodes(1, path)
            .map_err(ExternalHirReferenceSemanticValidationError::Resource)?;
        meter
            .charge_work(1, path)
            .map_err(ExternalHirReferenceSemanticValidationError::Resource)?;
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

        meter
            .check_table_entries(self.witnesses().witnesses().len() as u64, path)
            .map_err(ExternalHirReferenceSemanticValidationError::Resource)?;
        if !self.witnesses().is_empty() {
            let root = authority
                .external_hir_target_binding_root(self.target())
                .map_err(
                    |error| ExternalHirReferenceSemanticValidationError::BindingRoot {
                        target: self.target(),
                        error,
                    },
                )?;
            for (index, witness) in self.witnesses().witnesses().iter().enumerate() {
                witness
                    .validate_semantics(
                        root,
                        authority,
                        meter,
                        &path.clone().field(4).index(index as u64),
                    )
                    .map_err(
                        |error| ExternalHirReferenceSemanticValidationError::Witness {
                            index,
                            error,
                        },
                    )?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExternalHirReferenceSemanticValidationError<E> {
    Resource(WireError),
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
    BindingRoot {
        target: ExternalHirTargetV1,
        error: E,
    },
    Witness {
        index: usize,
        error: DependencyBindingWitnessSemanticValidationError,
    },
}

impl<E: fmt::Display> fmt::Display for ExternalHirReferenceSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
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
            Self::BindingRoot { target, error } => write!(
                formatter,
                "external HIR target {target:?} public binding root is unavailable: {error}"
            ),
            Self::Witness { index, error } => {
                write!(
                    formatter,
                    "invalid dependency binding witness {index}: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExternalHirReferenceSemanticValidationError<E>
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::TargetOrigin { error, .. } | Self::BindingRoot { error, .. } => Some(error),
            Self::Witness { error, .. } => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
