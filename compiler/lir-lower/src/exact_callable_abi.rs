//! Callable ABI publication from the same materialized MIR and LIR bodies.

use scoop_identity::CallableDefinitionOwner;
use scoop_lir as lir;
use scoop_mir as mir;

/// Ordinary and lowered callables share the canonical signature projection.
pub fn lower_exact_callable_abi_export(
    input: &mir::ConeMirInput,
    output: &lir::ConeLirOutput,
    target: impl Into<CallableDefinitionOwner>,
    signature: &mir::MirBridgeCallableSignatureV1,
) -> Result<lir::ExactCallableAbiExportV1, ExactCallableAbiLoweringError> {
    let target = target.into();
    if input.module().cone != output.foundation().producer() {
        return Err(ExactCallableAbiLoweringError::Provider);
    }
    let materialized = crate::callable_abi::LocalCallableMaterialization::resolve(
        input,
        &output.module().functions,
        target,
        signature.exact(),
    )
    .map_err(ExactCallableAbiLoweringError::Materialization)?;
    if materialized.mir.gc_effect != signature.gc_effect() {
        return Err(ExactCallableAbiLoweringError::GcEffect);
    }
    let abi = materialized
        .canonical_signature(&output.module().enums)
        .map_err(ExactCallableAbiLoweringError::Materialization)?;
    lir::ExactCallableAbiExportV1::from_signature(
        output.module().meta.target_profile,
        target,
        abi,
        output.foundation(),
    )
    .map_err(ExactCallableAbiLoweringError::Abi)
}

#[derive(Debug)]
pub enum ExactCallableAbiLoweringError {
    Provider,
    GcEffect,
    Materialization(crate::CallableAbiProjectionError),
    Abi(lir::ExactCallableAbiError),
}
impl std::fmt::Display for ExactCallableAbiLoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "exact callable ABI projection failed: {self:?}")
    }
}
impl std::error::Error for ExactCallableAbiLoweringError {}
