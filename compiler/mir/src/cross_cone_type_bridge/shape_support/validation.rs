use super::*;
use std::fmt;

#[derive(Debug)]
pub enum MirShapeSupportError {
    Reference(IdentityReferenceError),
    Resource(WireError),
    MissingType { exact: PersistentExactTypeId },
    InvalidSource { source: PersistentTypeId },
    BoxAvailability { source: PersistentTypeId },
    HelperRole { exact: PersistentExactTypeId },
    HelperGc { exact: PersistentExactTypeId },
    ProviderMismatch { source: PersistentTypeId },
    DuplicateSource { source: PersistentTypeId },
    NonCanonicalSourceOrder { index: usize },
    NonCanonicalRequiredSources { index: usize },
    MissingSource { source: PersistentTypeId },
    UnexpectedSource { source: PersistentTypeId },
}
impl From<IdentityReferenceError> for MirShapeSupportError {
    fn from(value: IdentityReferenceError) -> Self {
        Self::Reference(value)
    }
}
impl From<WireError> for MirShapeSupportError {
    fn from(value: WireError) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for MirShapeSupportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "MIR shape support: {self:?}")
    }
}
impl std::error::Error for MirShapeSupportError {}

impl<'a> MirShapeSupportAuthority<'a> {
    pub(in crate::cross_cone_type_bridge) fn validate(
        self,
        record: &ParamFreeMirShapeSupportV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), MirShapeSupportError> {
        let lookup_work = usize::BITS - self.types.record_count().leading_zeros();
        meter.charge_work(8 * u64::from(lookup_work + 1), &WirePath::root())?;
        let source = self.get(record.exact)?;
        if source.origin() != &MirTypeOriginV1::SourceNominal(record.source) {
            return Err(MirShapeSupportError::InvalidSource {
                source: record.source,
            });
        }
        match (source.facts().kind(), record.boxed) {
            (MirValueKindV1::Reference, MirBoxedShapeSupportV1::ReferenceNominalRequiresNoBox) => {}
            (
                MirValueKindV1::ZeroSizedValue | MirValueKindV1::NonZeroValue,
                MirBoxedShapeSupportV1::Available(exact),
            ) => {
                let boxed = self.helper(
                    exact,
                    &GeneratedNominalKey::BoxedValue {
                        payload: record.exact,
                    },
                )?;
                if !matches!(boxed.representation(), MirTypeRepresentationV1::BoxedValue { payload } if payload.value == record.exact)
                {
                    return Err(MirShapeSupportError::HelperRole { exact });
                }
            }
            _ => {
                return Err(MirShapeSupportError::BoxAvailability {
                    source: record.source,
                });
            }
        }
        let step = self.helper(
            record.coroutine_step,
            &GeneratedNominalKey::CoroutineStep {
                result: record.exact,
            },
        )?;
        let slot = self.helper(
            record.coroutine_slot,
            &GeneratedNominalKey::CoroutineSlot {
                value: record.exact,
            },
        )?;
        let (
            MirTypeRepresentationV1::CoroutineStep {
                variants: step_variants,
            },
            MirTypeRepresentationV1::CoroutineSlot {
                variants: slot_variants,
            },
        ) = (step.representation(), slot.representation())
        else {
            return Err(MirShapeSupportError::HelperRole {
                exact: record.exact,
            });
        };
        validate_variants(step, step_variants, source.facts().gc(), 0)?;
        validate_variants(slot, slot_variants, source.facts().gc(), 1)
    }
    fn get(
        self,
        exact: PersistentExactTypeId,
    ) -> Result<&'a ParamFreeMirTypeExportV1, MirShapeSupportError> {
        self.types
            .get(exact)
            .ok_or(MirShapeSupportError::MissingType { exact })
    }
    fn helper(
        self,
        exact: PersistentExactTypeId,
        expected: &GeneratedNominalKey,
    ) -> Result<&'a ParamFreeMirTypeExportV1, MirShapeSupportError> {
        let record = self.get(exact)?;
        if !matches!(record.origin(), MirTypeOriginV1::GeneratedNominal { role, .. } if role == expected)
        {
            return Err(MirShapeSupportError::HelperRole { exact });
        }
        Ok(record)
    }
}

fn validate_variants(
    helper: &ParamFreeMirTypeExportV1,
    variants: &[MirRepresentationVariantV1],
    gc: MirGcKindV1,
    payload_index: usize,
) -> Result<(), MirShapeSupportError> {
    if variants.len() != 2
        || helper.facts().gc() != gc
        || variants[payload_index].gc != gc
        || variants[1 - payload_index].gc != MirGcKindV1::GcFree
    {
        return Err(MirShapeSupportError::HelperGc {
            exact: helper.exact(),
        });
    }
    Ok(())
}
