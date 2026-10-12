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
        CBridgeToolchainProfileId::darwin_aarch64_apple_clang(),
        contract,
    )
}

pub fn linux_c_bridge_profile(
    target: LirTargetProfile,
    compiler: GccCompilerIdentityV1,
) -> Result<CBridgeToolchainProfileV1, CBridgeToolchainBuildError> {
    let id = match target.id() {
        TargetProfileId::LinuxX86_64Gnu => CBridgeToolchainProfileId::linux_x86_64_gnu_gcc(),
        TargetProfileId::LinuxX86_64Musl => CBridgeToolchainProfileId::linux_x86_64_musl_gcc(),
        TargetProfileId::DarwinAarch64 => {
            return Err(CBridgeToolchainBuildError::Platform(
                CBridgePlatformError::ExpectedLinux,
            ));
        }
    };
    let contract = CBridgeToolchainContractV1::new(
        target,
        CBridgePlatformContractV1::Linux { compiler },
        CanonicalCBridgeFlagContractV1::LINUX_GCC,
    )
    .map_err(CBridgeToolchainBuildError::Hash)?;
    CBridgeToolchainProfileV1::new(id, contract).map_err(CBridgeToolchainBuildError::Hash)
}
