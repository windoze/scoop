use std::fmt;

use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::{CrossConeHirInterfaceSectionV1, signature_nominal_walk::SignatureNominalWalker};
use crate::{
    ExternalHirReferenceRoleV1, ExternalHirReferenceSemanticAuthority, ExternalHirTargetV1,
    TypeAliasTargetV1,
};

impl CrossConeHirInterfaceSectionV1 {
    /// Validates that `AliasTarget` is exactly the set of foreign targets used
    /// by field 5 type-alias edges and expanded alias signatures.
    pub fn validate_alias_reference_closure<A, E>(
        &self,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), ExternalHirAliasClosureValidationError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let references = self.external_references();
        let references_path = path.clone().field(10);
        let mut seen = Vec::new();
        meter
            .try_reserve_collection_slots(&mut seen, references.records().len(), &references_path)
            .map_err(ExternalHirAliasClosureValidationError::Resource)?;
        seen.resize(references.records().len(), false);
        let current = authority.current_cone();

        for (wire_index, (record_index, record)) in
            (0_u64..).zip(self.type_aliases().records().iter().enumerate())
        {
            let target_path = path.clone().field(5).index(wire_index).field(2).field(1);
            match record.target() {
                TypeAliasTargetV1::Alias(alias) => observe_alias_target(
                    references,
                    &mut seen,
                    authority,
                    current,
                    ExternalHirTargetV1::TypeAlias(*alias),
                    ExternalHirAliasUseSiteV1::DirectAlias { record_index },
                    meter,
                    &target_path,
                )?,
                TypeAliasTargetV1::Signature(signature) => {
                    let site = ExternalHirAliasUseSiteV1::ExpandedSignature { record_index };
                    let mut walker = SignatureNominalWalker::new(signature, meter, &target_path)
                        .map_err(ExternalHirAliasClosureValidationError::Resource)?;
                    while let Some(declaration) = walker
                        .next(meter, &target_path)
                        .map_err(ExternalHirAliasClosureValidationError::Resource)?
                    {
                        observe_alias_target(
                            references,
                            &mut seen,
                            authority,
                            current,
                            ExternalHirTargetV1::from(declaration),
                            site,
                            meter,
                            &target_path,
                        )?;
                    }
                }
            }
        }

        for (record_index, record) in references.records().iter().enumerate() {
            meter
                .charge_work(1, &references_path)
                .map_err(ExternalHirAliasClosureValidationError::Resource)?;
            if record
                .roles()
                .contains(ExternalHirReferenceRoleV1::AliasTarget)
                && !seen[record_index]
            {
                return Err(ExternalHirAliasClosureValidationError::ExtraRole {
                    record_index,
                    target: record.target(),
                });
            }
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn observe_alias_target<A, E>(
    references: &crate::CanonicalExternalHirReferencesV1,
    seen: &mut [bool],
    authority: &mut A,
    current: scoop_identity::ConeIdentity,
    target: ExternalHirTargetV1,
    site: ExternalHirAliasUseSiteV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), ExternalHirAliasClosureValidationError<E>>
where
    A: ExternalHirReferenceSemanticAuthority<E>,
{
    let expected = authority
        .external_hir_target_origin(target)
        .map_err(
            |error| ExternalHirAliasClosureValidationError::TargetOrigin {
                site,
                target,
                error,
            },
        )?;
    if expected == current {
        return Ok(());
    }

    let record_index = references
        .find_index_metered(target, meter, path)
        .map_err(ExternalHirAliasClosureValidationError::Resource)?
        .ok_or(ExternalHirAliasClosureValidationError::MissingReference { site, target })?;
    let record = &references.records()[record_index];
    if record.origin() != expected {
        return Err(ExternalHirAliasClosureValidationError::OriginMismatch {
            site,
            record_index,
            target,
            expected,
            actual: record.origin(),
        });
    }
    if !record
        .roles()
        .contains(ExternalHirReferenceRoleV1::AliasTarget)
    {
        return Err(ExternalHirAliasClosureValidationError::MissingRole {
            site,
            record_index,
            target,
        });
    }
    seen[record_index] = true;
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalHirAliasUseSiteV1 {
    DirectAlias { record_index: usize },
    ExpandedSignature { record_index: usize },
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExternalHirAliasClosureValidationError<E> {
    TargetOrigin {
        site: ExternalHirAliasUseSiteV1,
        target: ExternalHirTargetV1,
        error: E,
    },
    MissingReference {
        site: ExternalHirAliasUseSiteV1,
        target: ExternalHirTargetV1,
    },
    OriginMismatch {
        site: ExternalHirAliasUseSiteV1,
        record_index: usize,
        target: ExternalHirTargetV1,
        expected: scoop_identity::ConeIdentity,
        actual: scoop_identity::ConeIdentity,
    },
    MissingRole {
        site: ExternalHirAliasUseSiteV1,
        record_index: usize,
        target: ExternalHirTargetV1,
    },
    ExtraRole {
        record_index: usize,
        target: ExternalHirTargetV1,
    },
    Resource(WireError),
}

impl<E: fmt::Display> fmt::Display for ExternalHirAliasClosureValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetOrigin {
                site,
                target,
                error,
            } => write!(
                formatter,
                "type-alias target {target:?} used at {site:?} has no canonical origin: {error}"
            ),
            Self::MissingReference { site, target } => write!(
                formatter,
                "foreign type-alias target {target:?} used at {site:?} is absent from the external HIR reference table"
            ),
            Self::OriginMismatch {
                site,
                record_index,
                target,
                expected,
                actual,
            } => write!(
                formatter,
                "type-alias target {target:?} used at {site:?} uses external record {record_index} with origin {actual}, expected {expected}"
            ),
            Self::MissingRole {
                site,
                record_index,
                target,
            } => write!(
                formatter,
                "type-alias target {target:?} used at {site:?} uses external record {record_index} without AliasTarget role"
            ),
            Self::ExtraRole {
                record_index,
                target,
            } => write!(
                formatter,
                "external record {record_index} for target {target:?} has AliasTarget role without a type-alias use in field 5"
            ),
            Self::Resource(error) => write!(
                formatter,
                "external type-alias reference closure resource failure: {error}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExternalHirAliasClosureValidationError<E>
{
}

#[cfg(test)]
mod tests;
