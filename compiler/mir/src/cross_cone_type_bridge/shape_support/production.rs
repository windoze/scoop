use super::*;
use crate::{SingleConeStrongMirInput, StrongBoxedShapeSupportRoot};

impl CanonicalMirShapeSupportsV1 {
    /// Produces the semantic support families from the same complete bindings
    /// used to export actual MIR helper types. No helper is reconstructed.
    pub fn from_strong_input(
        input: &SingleConeStrongMirInput,
        identities: &ValidatedIdentityGraph,
        types: &CanonicalParamFreeMirTypeExportsV1,
        meter: &mut BudgetMeter,
    ) -> Result<Self, MirShapeSupportError> {
        let roots = input.materialization().shape_support();
        let path = WirePath::root();
        meter.check_table_entries(roots.len() as u64, &path)?;
        meter.charge_owned_bytes(
            (roots.len() as u64)
                .saturating_mul(std::mem::size_of::<ParamFreeMirShapeSupportV1>() as u64),
            &path,
        )?;
        let mut records = Vec::new();
        meter.try_reserve_collection_slots(&mut records, roots.len(), &path)?;
        let authority = MirShapeSupportAuthority { identities, types };
        for root in roots {
            meter.charge_nodes(1, &path)?;
            let boxed = match root.boxed() {
                StrongBoxedShapeSupportRoot::Available(boxed) => {
                    MirBoxedShapeSupportV1::Available(boxed.exact())
                }
                StrongBoxedShapeSupportRoot::ReferenceNominalRequiresNoBox => {
                    MirBoxedShapeSupportV1::ReferenceNominalRequiresNoBox
                }
            };
            records.push(ParamFreeMirShapeSupportV1::try_new(
                authority,
                root.shape().source(),
                root.shape().exact(),
                boxed,
                root.coroutine_step().exact(),
                root.coroutine_slot().exact(),
                meter,
            )?);
        }
        Self::try_new(input.module().cone, authority, records, meter)
    }
}
