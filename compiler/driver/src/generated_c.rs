//! Driver-owned generated-C compilation orchestration.

use std::path::Path;

use scoop_codegen::{CodegenError, EmittedGeneratedCBridgeObjectSetV1};
use scoop_toolchain::ValidatedCBridgeToolchainInvocation;

pub(crate) fn emit_c_bridge_object_set(
    input: &scoop_lir::ConeLirOutput,
    temporary_parent: &Path,
    invocation: &ValidatedCBridgeToolchainInvocation,
) -> Result<EmittedGeneratedCBridgeObjectSetV1, CodegenError> {
    invocation
        .validate_target(input.module().meta.target_profile)
        .map_err(|error| {
            CodegenError(format!("invalid generated-C toolchain projection: {error}"))
        })?;
    scoop_codegen::emit_c_bridge_object_set(
        input,
        temporary_parent,
        invocation.profile(),
        |source, object| {
            invocation
                .compile_object(source, object)
                .map_err(|error| CodegenError(error.to_string()))
        },
    )
}
