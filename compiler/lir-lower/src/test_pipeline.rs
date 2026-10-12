use crate::{LirLoweringError, StorageLoweringError, prepare};
use scoop_lir as lir;
use scoop_mir as mir;

pub(crate) fn lower(
    input: &mir::ConeMirInput,
    descriptors: &[lir::ExternalTypeDescriptor],
    callables: &lir::SelectedExternalLirSet,
    target: lir::LirTargetProfile,
) -> Result<lir::ConeLirOutput, LirLoweringError> {
    let prepared = prepare(input, descriptors, callables, target, None)?;
    let plans = scoop_abi_plan::plan(prepared.abi_input()).map_err(StorageLoweringError::from)?;
    prepared.complete(plans)
}

impl From<scoop_abi_plan::AbiPlanError> for StorageLoweringError {
    fn from(error: scoop_abi_plan::AbiPlanError) -> Self {
        match error {
            scoop_abi_plan::AbiPlanError::Value(error) => Self::AbiValue(error),
            scoop_abi_plan::AbiPlanError::UnsupportedTarget(_) => {
                Self::InvalidRepresentation("ABI planner does not support the test target")
            }
        }
    }
}
