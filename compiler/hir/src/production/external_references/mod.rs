//! Canonical production of the cross-Cone external HIR reference closure.

use scoop_wire::{BudgetMeter, DecodeLimits};

use crate::{CanonicalExternalHirReferencesV1, ExternalHirReferenceSemanticAuthority};

mod accumulator;
mod defaults;
mod errors;
mod input;
mod signatures;
mod surface;

pub use errors::ExternalHirReferenceProductionError;
pub use input::{
    ExternalHirBindingWitnessRole, ExternalHirBindingWitnessUse,
    ExternalHirReferenceProductionInput,
};

impl CanonicalExternalHirReferencesV1 {
    /// Builds the exact canonical union of all foreign typed references in
    /// interface fields 1 through 8 plus committed concrete selected uses.
    pub fn from_interface_parts<A, E>(
        input: ExternalHirReferenceProductionInput<'_>,
        witness_uses: &[ExternalHirBindingWitnessUse],
        authority: &mut A,
    ) -> Result<Self, ExternalHirReferenceProductionError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        Self::from_interface_parts_with_core(input, witness_uses, None, authority)
    }

    pub(crate) fn from_interface_parts_with_core<A, E>(
        input: ExternalHirReferenceProductionInput<'_>,
        witness_uses: &[ExternalHirBindingWitnessUse],
        imported_core: Option<&crate::SelectedImportedCoreSet<'_>>,
        authority: &mut A,
    ) -> Result<Self, ExternalHirReferenceProductionError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let mut meter = BudgetMeter::new(DecodeLimits::default());
        let mut accumulator = accumulator::ExternalReferenceAccumulator::new(authority);

        surface::collect_reexports(input, &mut accumulator)?;
        signatures::collect(input, &mut accumulator, &mut meter)?;
        surface::collect_aliases(input, &mut accumulator, &mut meter)?;
        defaults::collect(input, &mut accumulator, &mut meter)?;
        surface::collect_constants(input, &mut accumulator, &mut meter)?;
        for use_ in witness_uses {
            accumulator.add_witness_use(use_)?;
        }
        if let Some(core) = imported_core {
            accumulator.add_implicit_core_witnesses(core);
        }

        accumulator.finish()
    }
}

#[cfg(test)]
mod tests;
