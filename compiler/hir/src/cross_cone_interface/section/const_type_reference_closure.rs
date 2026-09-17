use std::fmt;

use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::{CrossConeHirInterfaceSectionV1, signature_nominal_walk::SignatureNominalWalker};
use crate::{
    ExternalHirReferenceRoleV1, ExternalHirReferenceSemanticAuthority, ExternalHirTargetV1,
};

impl CrossConeHirInterfaceSectionV1 {
    /// Validates that `ConstType` is exactly the set of foreign nominal leaves
    /// used by field 8 constant value types.
    pub fn validate_const_type_reference_closure<A, E>(
        &self,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), ExternalHirConstTypeClosureValidationError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let references = self.external_references();
        let references_path = path.clone().field(10);
        let mut seen = Vec::new();
        meter
            .try_reserve_collection_slots(&mut seen, references.records().len(), &references_path)
            .map_err(ExternalHirConstTypeClosureValidationError::Resource)?;
        seen.resize(references.records().len(), false);
        let current = authority.current_cone();

        for (wire_index, (constant_index, constant)) in
            (0_u64..).zip(self.constants().records().iter().enumerate())
        {
            let signature_path = path.clone().field(8).index(wire_index).field(2);
            let mut walker =
                SignatureNominalWalker::new(constant.value_type(), meter, &signature_path)
                    .map_err(ExternalHirConstTypeClosureValidationError::Resource)?;
            while let Some(declaration) = walker
                .next(meter, &signature_path)
                .map_err(ExternalHirConstTypeClosureValidationError::Resource)?
            {
                let target = ExternalHirTargetV1::from(declaration);
                let expected = authority
                    .external_hir_target_origin(target)
                    .map_err(
                        |error| ExternalHirConstTypeClosureValidationError::TargetOrigin {
                            constant_index,
                            target,
                            error,
                        },
                    )?;
                if expected == current {
                    continue;
                }

                let record_index = references
                    .find_index_metered(target, meter, &signature_path)
                    .map_err(ExternalHirConstTypeClosureValidationError::Resource)?
                    .ok_or(
                        ExternalHirConstTypeClosureValidationError::MissingReference {
                            constant_index,
                            target,
                        },
                    )?;
                let record = &references.records()[record_index];
                if record.origin() != expected {
                    return Err(ExternalHirConstTypeClosureValidationError::OriginMismatch {
                        constant_index,
                        record_index,
                        target,
                        expected,
                        actual: record.origin(),
                    });
                }
                if !record
                    .roles()
                    .contains(ExternalHirReferenceRoleV1::ConstType)
                {
                    return Err(ExternalHirConstTypeClosureValidationError::MissingRole {
                        constant_index,
                        record_index,
                        target,
                    });
                }
                seen[record_index] = true;
            }
        }

        for (record_index, record) in references.records().iter().enumerate() {
            meter
                .charge_work(1, &references_path)
                .map_err(ExternalHirConstTypeClosureValidationError::Resource)?;
            if record
                .roles()
                .contains(ExternalHirReferenceRoleV1::ConstType)
                && !seen[record_index]
            {
                return Err(ExternalHirConstTypeClosureValidationError::ExtraRole {
                    record_index,
                    target: record.target(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExternalHirConstTypeClosureValidationError<E> {
    TargetOrigin {
        constant_index: usize,
        target: ExternalHirTargetV1,
        error: E,
    },
    MissingReference {
        constant_index: usize,
        target: ExternalHirTargetV1,
    },
    OriginMismatch {
        constant_index: usize,
        record_index: usize,
        target: ExternalHirTargetV1,
        expected: scoop_identity::ConeIdentity,
        actual: scoop_identity::ConeIdentity,
    },
    MissingRole {
        constant_index: usize,
        record_index: usize,
        target: ExternalHirTargetV1,
    },
    ExtraRole {
        record_index: usize,
        target: ExternalHirTargetV1,
    },
    Resource(WireError),
}

impl<E: fmt::Display> fmt::Display for ExternalHirConstTypeClosureValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetOrigin {
                constant_index,
                target,
                error,
            } => write!(
                formatter,
                "constant {constant_index} type target {target:?} has no canonical origin: {error}"
            ),
            Self::MissingReference {
                constant_index,
                target,
            } => write!(
                formatter,
                "foreign constant {constant_index} type target {target:?} is absent from the external HIR reference table"
            ),
            Self::OriginMismatch {
                constant_index,
                record_index,
                target,
                expected,
                actual,
            } => write!(
                formatter,
                "constant {constant_index} type target {target:?} uses external record {record_index} with origin {actual}, expected {expected}"
            ),
            Self::MissingRole {
                constant_index,
                record_index,
                target,
            } => write!(
                formatter,
                "constant {constant_index} type target {target:?} uses external record {record_index} without ConstType role"
            ),
            Self::ExtraRole {
                record_index,
                target,
            } => write!(
                formatter,
                "external record {record_index} for target {target:?} has ConstType role without a constant value type use"
            ),
            Self::Resource(error) => {
                write!(
                    formatter,
                    "external const type closure resource failure: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExternalHirConstTypeClosureValidationError<E>
{
}

#[cfg(test)]
mod tests;
