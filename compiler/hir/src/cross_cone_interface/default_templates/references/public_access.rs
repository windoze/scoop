use super::*;
use crate::{CallableInterfaceRecordV1, PublicLookupAccessV1};
use scoop_wire::{BudgetMeter, WireError, WirePath};

impl ExportDefaultReferenceSetV1 {
    /// Narrows source snapshots to the public domain required by a public
    /// publisher. Actual target declarations are validated independently.
    pub fn validate_public_access(
        &self,
        owner: &CallableInterfaceRecordV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), ExportDefaultPublicWitnessValidationError> {
        check(
            self.callables(),
            ExportDefaultReferenceKindV1::Callable,
            owner,
            meter,
            path,
        )?;
        check(
            self.constructors(),
            ExportDefaultReferenceKindV1::Constructor,
            owner,
            meter,
            path,
        )?;
        check(
            self.types(),
            ExportDefaultReferenceKindV1::Type,
            owner,
            meter,
            path,
        )?;
        check(
            self.globals(),
            ExportDefaultReferenceKindV1::Global,
            owner,
            meter,
            path,
        )?;
        check(
            self.singleton_values(),
            ExportDefaultReferenceKindV1::Singleton,
            owner,
            meter,
            path,
        )?;
        check(
            self.fields(),
            ExportDefaultReferenceKindV1::Field,
            owner,
            meter,
            path,
        )
    }
}

fn check<T>(
    references: &[ExportDefaultReferenceV1<T>],
    kind: ExportDefaultReferenceKindV1,
    owner: &CallableInterfaceRecordV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), ExportDefaultPublicWitnessValidationError> {
    let domain = match owner.access() {
        PublicLookupAccessV1::DirectOnly => ExportDefaultCallDomainV1::DirectPublic,
        PublicLookupAccessV1::PublicSlot => ExportDefaultCallDomainV1::DirectAndPublicSlot,
    };
    meter.check_table_entries(references.len() as u64, path)?;
    for (index, reference) in references.iter().enumerate() {
        meter.charge_work(1, path)?;
        reference
            .witness()
            .validate_public_access(owner.declaration(), domain)
            .map_err(|source| ExportDefaultPublicWitnessValidationError::Record {
                kind,
                index,
                source,
            })?;
    }
    Ok(())
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefaultPublicWitnessValidationError {
    Record {
        kind: ExportDefaultReferenceKindV1,
        index: usize,
        source: PublicDefaultWitnessError,
    },
    Resource(WireError),
}
impl From<WireError> for ExportDefaultPublicWitnessValidationError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for ExportDefaultPublicWitnessValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Record {
                kind,
                index,
                source,
            } => write!(f, "default {kind} reference {index}: {source}"),
            Self::Resource(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for ExportDefaultPublicWitnessValidationError {}
