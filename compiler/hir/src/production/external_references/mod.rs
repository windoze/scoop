//! Canonical production of the cross-Cone external HIR reference closure.

use crate::{CanonicalExternalHirReferencesV1, ExternalHirReferenceSemanticAuthority};

mod accumulator;
mod calls;
mod declaration_types;
mod defaults;
mod dispatch;
mod errors;
mod input;
mod origins;
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
        Self::from_interface_parts_with_dependencies(input, witness_uses, None, authority)
    }

    pub(crate) fn from_interface_parts_with_dependencies<A, E>(
        input: ExternalHirReferenceProductionInput<'_>,
        witness_uses: &[ExternalHirBindingWitnessUse],
        dependency_output: Option<&crate::DependencyHirOutput>,
        authority: &mut A,
    ) -> Result<Self, ExternalHirReferenceProductionError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let mut accumulator = accumulator::ExternalReferenceAccumulator::new(authority);

        surface::collect_reexports(input, &mut accumulator)?;
        signatures::collect(input, &mut accumulator)?;
        dispatch::collect(input, &mut accumulator)?;
        surface::collect_aliases(input, &mut accumulator)?;
        defaults::collect(input, &mut accumulator)?;
        for (index, body) in input.generic_callable_bodies.records().iter().enumerate() {
            body.visit_declaration_targets(
                &mut |target| {
                    accumulator
                        .observe(
                            target,
                            crate::ExternalHirReferenceRoleV1::TemplateDependency,
                        )
                        .map(|_| ())
                },
                &scoop_wire::WirePath::root().field(11).index(index as u64),
            )?;
        }
        for (index, initialization) in input.generic_initializations.records().iter().enumerate() {
            initialization.visit_declaration_targets(
                &mut |target| {
                    accumulator
                        .observe(
                            target,
                            crate::ExternalHirReferenceRoleV1::TemplateDependency,
                        )
                        .map(|_| ())
                },
                &scoop_wire::WirePath::root().field(12).index(index as u64),
            )?;
        }
        surface::collect_constants(input, &mut accumulator)?;
        for use_ in witness_uses {
            accumulator.add_witness_use(use_)?;
        }
        if let Some(output) = dependency_output {
            accumulator.add_call_sites(output)?;
            accumulator.add_type_sites(output)?;
            accumulator.add_declaration_type_sites(output)?;
        }

        accumulator.finish()
    }
}

impl<E> From<scoop_wire::WireError> for ExternalHirReferenceProductionError<E> {
    fn from(error: scoop_wire::WireError) -> Self {
        Self::Resource(error)
    }
}

#[cfg(test)]
mod tests;
