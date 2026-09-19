//! Exact body occurrence closure, separate from target publication authority.
use scoop_wire::{BudgetMeter, WirePath};

use super::ProtectedDefaultReferenceSetV1;
use crate::{
    CanonicalTemplateLocalTableV1, DefaultBodyReferenceOccurrenceV1, ExportDefaultBodyV1,
    ExportDefinitionSourceV1, OptionalTemplateReceiverV1, ProtectedDefaultAccessWitnessV1,
    ProtectedDefaultTemplateKeyV1,
};

mod collect;
mod domains;
mod errors;
mod receiver;
mod records;

#[cfg(test)]
mod tests;

pub use errors::*;
pub use receiver::ProtectedDefaultReferenceReceiverV1;

/// Replays each actual occurrence against independent definition-side source,
/// origin, access, receiver and whole-default domain authorities. Metadata
/// occurrences require the same checks even though they have no expression use.
/// Implementations must not derive those authorities from the candidate records.
pub trait ProtectedDefaultReferenceBodySemanticAuthority<E> {
    fn validate_default_reference_occurrence(
        &mut self,
        key: ProtectedDefaultTemplateKeyV1,
        occurrence: DefaultBodyReferenceOccurrenceV1<'_>,
        witness: &ProtectedDefaultAccessWitnessV1,
        receiver: ProtectedDefaultReferenceReceiverV1<'_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), E>;
}

/// Proves only the exact body/reference/use relation and successful occurrence
/// replay. Enclosing source, body typing and default-table validation remain
/// necessary; this handle cannot authorize selection or default expansion.
#[derive(Debug)]
pub struct CheckedProtectedDefaultReferenceBodyV1<'a> {
    references: &'a ProtectedDefaultReferenceSetV1,
}
impl CheckedProtectedDefaultReferenceBodyV1<'_> {
    pub const fn references(&self) -> &ProtectedDefaultReferenceSetV1 {
        self.references
    }
}

impl ProtectedDefaultReferenceSetV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn validate_body_closure<'a, A: ProtectedDefaultReferenceBodySemanticAuthority<E>, E>(
        &'a self,
        key: ProtectedDefaultTemplateKeyV1,
        body: &ExportDefaultBodyV1,
        locals: &CanonicalTemplateLocalTableV1,
        definition_origin: &ExportDefinitionSourceV1,
        receiver: &OptionalTemplateReceiverV1,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<CheckedProtectedDefaultReferenceBodyV1<'a>, ProtectedDefaultBodyClosureError<E>>
    {
        let collected = collect::collect(body, locals, definition_origin, meter, path)?;
        let mut domains = domains::Domains::new(self, meter, path)?;
        for occurrence in collected.occurrences.iter().copied() {
            domains.observe(
                key, occurrence, receiver, &collected, authority, meter, path,
            )?;
        }
        domains.finish(meter, path)?;
        Ok(CheckedProtectedDefaultReferenceBodyV1 { references: self })
    }
}
