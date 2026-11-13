use scoop::{BuildFailure, BuildFailurePhase, BuildResult};
use scoop_protocol::TargetSelectionRequestV1;
use scoop_toolchain::CToolchainOptions;

pub(super) fn target_request(
    triple: String,
    tools: &CToolchainOptions,
) -> BuildResult<TargetSelectionRequestV1> {
    tools
        .request(triple)
        .map_err(|error| BuildFailure::tool("SCOOP_CLI_CONFIG", BuildFailurePhase::Request, error))
}
