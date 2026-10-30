//! Contracts selected by the actual generated-C producer; no host invocation.

use scoop_identity::CBridgeToolchainProfileId;
use scoop_lir::*;

pub const fn darwin_c_bridge_flags(target: LirTargetProfile) -> CanonicalCBridgeFlagContractV1 {
    match target.architecture() {
        TargetArchitecture::Aarch64 => CanonicalCBridgeFlagContractV1::DarwinAppleClang,
        TargetArchitecture::X86_64 => CanonicalCBridgeFlagContractV1::DarwinAppleClangX86_64,
    }
}

pub fn darwin_c_bridge_profile(
    target: LirTargetProfile,
    deployment: DarwinCBridgeDeploymentContractV1,
    compiler: AppleClangCompilerIdentityV1,
) -> Result<CBridgeToolchainProfileV1, CBridgeToolchainBuildError> {
    if !matches!(
        crate::definition(target.id()).map(|definition| definition.native_environment),
        Some(crate::NativeEnvironment::Darwin)
    ) {
        return Err(CBridgeToolchainBuildError::Platform(
            CBridgePlatformError::ExpectedDarwin,
        ));
    }
    let contract = CBridgeToolchainContractV1::new(
        target,
        CBridgePlatformContractV1::Darwin {
            deployment,
            compiler,
        },
        darwin_c_bridge_flags(target),
    )
    .map_err(CBridgeToolchainBuildError::Hash)?;
    CBridgeToolchainProfileV1::new(
        c_bridge_toolchain_id(target.id()).expect("registered Darwin C producer"),
        contract,
    )
    .map_err(CBridgeToolchainBuildError::Hash)
}

pub fn linux_c_bridge_profile(
    target: LirTargetProfile,
    compiler: LinuxCCompilerIdentityV1,
) -> Result<CBridgeToolchainProfileV1, CBridgeToolchainBuildError> {
    if !matches!(
        crate::definition(target.id()).map(|definition| definition.native_environment),
        Some(crate::NativeEnvironment::LinuxGnu | crate::NativeEnvironment::LinuxMusl)
    ) {
        return Err(CBridgeToolchainBuildError::Platform(
            CBridgePlatformError::ExpectedLinux,
        ));
    }
    let id = c_bridge_toolchain_id(target.id()).expect("registered Linux C producer");
    let flags = match &compiler {
        LinuxCCompilerIdentityV1::Gcc(_) => CanonicalCBridgeFlagContractV1::LinuxGcc,
        LinuxCCompilerIdentityV1::Clang(_) => CanonicalCBridgeFlagContractV1::LinuxClang,
    };
    let contract = CBridgeToolchainContractV1::new(
        target,
        CBridgePlatformContractV1::Linux { compiler },
        flags,
    )
    .map_err(CBridgeToolchainBuildError::Hash)?;
    CBridgeToolchainProfileV1::new(id, contract).map_err(CBridgeToolchainBuildError::Hash)
}

pub fn c_bridge_toolchain_id(target: TargetProfileId) -> Option<CBridgeToolchainProfileId> {
    crate::definition(target).map(|target| {
        CBridgeToolchainProfileId::new(target.c_bridge_profile, 1).expect("registered C producer")
    })
}

pub fn windows_c_bridge_profile(
    target: LirTargetProfile,
    compiler: ClangCompilerIdentityV1,
) -> Result<CBridgeToolchainProfileV1, CBridgeToolchainBuildError> {
    if !matches!(
        crate::definition(target.id()).map(|definition| definition.native_environment),
        Some(
            crate::NativeEnvironment::MicrosoftUcrt
                | crate::NativeEnvironment::LlvmMingwMsvcrt
                | crate::NativeEnvironment::LlvmMingwUcrt
        )
    ) {
        return Err(CBridgeToolchainBuildError::Platform(
            CBridgePlatformError::ExpectedWindows,
        ));
    }
    let id = c_bridge_toolchain_id(target.id()).expect("registered Windows C producer");
    let contract = CBridgeToolchainContractV1::new(
        target,
        CBridgePlatformContractV1::Windows { compiler },
        CanonicalCBridgeFlagContractV1::WindowsClang,
    )
    .map_err(CBridgeToolchainBuildError::Hash)?;
    CBridgeToolchainProfileV1::new(id, contract).map_err(CBridgeToolchainBuildError::Hash)
}

#[cfg(test)]
mod tests;
