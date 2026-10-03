use super::*;
use crate::{ConeMirInput, StrongBoxedShapeSupportRoot};

impl CanonicalMirShapeSupportsV1 {
    /// Produces the semantic support families from the same complete bindings
    /// used to export actual MIR helper types. No helper is reconstructed.
    pub fn from_strong_input(
        input: &ConeMirInput,
        identities: &ValidatedIdentityGraph,
        types: &CanonicalParamFreeMirTypeExportsV1,
    ) -> Result<Self, MirShapeSupportError> {
        let roots = input.materialization().shape_support();
        let path = WirePath::root();

        let mut records = Vec::new();
        scoop_wire::allocation::try_reserve(&mut records, roots.len(), &path)?;
        let authority = MirShapeSupportAuthority { identities, types };
        for root in roots {
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
            )?);
        }
        Self::try_new(input.module().cone, authority, records)
    }
}
