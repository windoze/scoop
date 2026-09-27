//! Actual MIR projection of the finite source shape-support materializations.

use super::*;
use crate::{ConeMirInput, GeneratedExactTypeLocation, StrongBoxedShapeSupportRoot};

mod representation;

impl CanonicalParamFreeMirTypeExportsV1 {
    /// Produces the finite generated constituent of the complete type table.
    /// The sealed source plan determines membership; unrelated execution
    /// environments are not part of this export surface.
    pub fn from_finite_shape_support(
        input: &ConeMirInput,
        sources: &CanonicalParamFreeMirTypeExportsV1,
        identities: &ValidatedIdentityGraph,
    ) -> Result<Self, MirTypeBridgeError> {
        let plan = input.materialization();
        let authority = MirTypeBridgeAuthority {
            identities,
            foundation: input.foundation(),
        };
        let mut records = Vec::new();
        reserve(&mut records, plan.shape_support().len().saturating_mul(3))?;
        for root in plan.shape_support() {
            let exact = root.shape().exact();

            let source = sources
                .get(exact)
                .ok_or(MirTypeBridgeError::MissingShapeSupportSource { exact })?;
            if source.origin() != &MirTypeOriginV1::SourceNominal(root.shape().source()) {
                return Err(MirTypeBridgeError::ExactOriginMismatch { exact });
            }
            let boxed = match root.boxed() {
                StrongBoxedShapeSupportRoot::Available(boxed) => {
                    Some((boxed, GeneratedNominalKey::BoxedValue { payload: exact }))
                }
                StrongBoxedShapeSupportRoot::ReferenceNominalRequiresNoBox => None,
            };
            let helpers = [
                boxed,
                Some((
                    root.coroutine_step(),
                    GeneratedNominalKey::CoroutineStep { result: exact },
                )),
                Some((
                    root.coroutine_slot(),
                    GeneratedNominalKey::CoroutineSlot { value: exact },
                )),
            ];
            for (helper, role) in helpers.into_iter().flatten() {
                let (facts, representation, bases) =
                    representation::project(input, source, helper.location(), &role)?;
                records.push(ParamFreeMirTypeExportV1::try_new(
                    authority,
                    helper.exact(),
                    MirTypeOriginV1::GeneratedNominal {
                        nominal: helper.nominal(),
                        role,
                    },
                    facts,
                    representation,
                    bases,
                )?);
            }
        }

        Self::try_new(records)
    }
}

fn reserve<T>(values: &mut Vec<T>, count: usize) -> Result<(), MirTypeBridgeError> {
    let path = WirePath::root();

    scoop_wire::allocation::try_reserve(values, count, &path).map_err(MirTypeBridgeError::Resource)
}
