//! Contracts selected by the actual generated-C producer; no host invocation.

use scoop_identity::CBridgeToolchainProfileId;
use scoop_lir::*;

pub fn darwin_c_bridge_profile(
    deployment: DarwinCBridgeDeploymentContractV1,
    compiler: AppleClangCompilerIdentityV1,
) -> Result<CBridgeToolchainProfileV1, scoop_wire::HashError> {
    let contract = CBridgeToolchainContractV1::new(
        crate::DARWIN_AARCH64,
        CBridgePlatformContractV1::Darwin {
            deployment,
            compiler,
        },
        CanonicalCBridgeFlagContractV1::CURRENT,
    )?;
    CBridgeToolchainProfileV1::new(
        CBridgeToolchainProfileId::new("darwin-aarch64-apple-clang", 1).unwrap(),
        contract,
    )
}
